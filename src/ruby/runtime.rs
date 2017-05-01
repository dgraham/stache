pub const RUNTIME: &'static str = r#"
#include "ruby.h"
#include <stdbool.h>
#include <string.h>

static void html_escaped_cat(VALUE str, char c) {
    switch (c) {
        case '\'':
            rb_str_cat_cstr(str, "&#39;");
            break;
        case '&':
            rb_str_cat_cstr(str, "&amp;");
            break;
        case '"':
            rb_str_cat_cstr(str, "&quot;");
            break;
        case '<':
            rb_str_cat_cstr(str, "&lt;");
            break;
        case '>':
            rb_str_cat_cstr(str, "&gt;");
            break;
    }
}

static VALUE optimized_escape_html(VALUE str) {
    long beg = 0;
    VALUE dest = 0;

    const long len = RSTRING_LEN(str);
    const char *cstr = RSTRING_PTR(str);

    for (long i = 0; i < len; i++) {
        switch (cstr[i]) {
          case '\'':
          case '&':
          case '"':
          case '<':
          case '>':
            if (!dest) {
                dest = rb_str_buf_new(len);
            }

            rb_str_cat(dest, cstr + beg, i - beg);
            beg = i + 1;

            html_escaped_cat(dest, cstr[i]);
            break;
        }
    }

    if (dest) {
        rb_str_cat(dest, cstr + beg, len - beg);
        return dest;
    } else {
        return str;
    }
}

static const char *DOT = ".";

static ID id_to_s;
static ID id_miss;
static ID id_buf;
static VALUE Buffer;

struct stack {
    VALUE data;
    const struct stack *parent;
};

struct buffer {
    char *data;
    size_t capacity;
    size_t length;
};

bool buffer_init(struct buffer *this) {
    const size_t capacity = 2048;
    char *data = malloc(capacity);
    if (!data) {
        return false;
    }
    this->data = data;
    this->capacity = capacity;
    this->length = 0;
    return true;
}

void buffer_destroy(struct buffer *this) {
    free(this->data);
    this->data = NULL;
    this->capacity = 0;
    this->length = 0;
}

void buffer_clear(struct buffer *this) {
    this->length = 0;
}

bool buffer_resize(struct buffer *this, size_t capacity) {
    void *data = realloc(this->data, capacity);
    if (!data) {
        return false;
    }
    this->data = data;
    this->capacity = capacity;
    return true;
}

bool buffer_append(struct buffer *this, const char *value, size_t length) {
    size_t min = this->length + length;
    if (this->capacity < min) {
        size_t ideal = this->capacity * 2;
        size_t capacity = (min < ideal) ? ideal : min * 1.1;
        if (!buffer_resize(this, capacity)) {
            return false;
        }
    }
    memcpy(this->data + this->length, value, length);
    this->length += length;
    return true;
}

struct path {
    char *keys[16];
    int length;
};

static VALUE fetch(VALUE context, const char *key) {
    if (strlen(key) == 1 && strncmp(key, DOT, 1) == 0) {
        return context;
    }

    switch (rb_type(context)) {
        case T_HASH: {
            VALUE miss = ID2SYM(id_miss);
            VALUE sym = ID2SYM(rb_intern(key));
            VALUE value = rb_hash_lookup2(context, sym, miss);
            if (value == miss) {
                VALUE key_str = rb_str_new_cstr(key);
                value = rb_hash_lookup2(context, key_str, miss);
                if (value == miss) {
                    value = Qundef;
                }
            }
            return value;
        }
        case T_FALSE:
            return Qfalse;
        case T_NIL:
        case T_UNDEF:
            return Qundef;
        default: {
            ID method = rb_intern(key);
            if (rb_respond_to(context, method)) {
                return rb_funcall(context, method, 0);
            } else {
                return Qundef;
            }
        }
    }
}

static VALUE context_fetch(const struct stack *stack, const char *key) {
    do {
        VALUE value = fetch(stack->data, key);
        if (value != Qundef) {
            return value;
        }
    } while ((stack = stack->parent));

    return Qundef;
}

static VALUE fetch_path(const struct stack *stack, const struct path *path) {
    VALUE value = context_fetch(stack, path->keys[0]);
    for (long i = 1; i < path->length; i++) {
        value = fetch(value, path->keys[i]);
    }
    return value;
}

static void append_value(struct buffer *buf, const struct stack *stack, const struct path *path, bool escape) {
    VALUE value = fetch_path(stack, path);
    switch (rb_type(value)) {
        case T_NIL:
        case T_UNDEF:
            return;
        case T_STRING:
            break;
        default:
            value = rb_funcall(value, id_to_s, 0);
            break;
    }

    value = escape ? optimized_escape_html(value) : value;

    if (!buffer_append(buf, RSTRING_PTR(value), RSTRING_LEN(value))) {
        buffer_clear(buf);
        rb_raise(rb_eRuntimeError, "Memory allocation failed");
    }
}

static void section(struct buffer *buf, const struct stack *stack, const struct path *path, void (*block)(struct buffer *, const struct stack *)) {
    VALUE value = fetch_path(stack, path);
    switch (rb_type(value)) {
        case T_ARRAY: {
            struct stack frame = { .parent = stack };
            for (long i = 0; i < RARRAY_LEN(value); i++) {
                frame.data = RARRAY_AREF(value, i);
                block(buf, &frame);
            }
            break;
        }
        case T_NIL:
        case T_UNDEF:
        case T_FALSE:
            break;
        case T_TRUE:
            block(buf, stack);
            break;
        default: {
            const struct stack frame = { .data = value, .parent = stack };
            block(buf, &frame);
            break;
        }
    }
}

static void inverted(struct buffer *buf, const struct stack *stack, const struct path *path, void (*block)(struct buffer *, const struct stack *)) {
    VALUE value = fetch_path(stack, path);
    switch (rb_type(value)) {
        case T_ARRAY:
            if (RARRAY_LEN(value) == 0) {
                block(buf, stack);
            }
            break;
        case T_NIL:
        case T_UNDEF:
        case T_FALSE:
            block(buf, stack);
            break;
    }
}

static VALUE render(VALUE self, VALUE name, VALUE context);

static void buffer_free(void *ptr) {
    buffer_destroy(ptr);
    free(ptr);
}

static size_t buffer_memsize(const void *ptr) {
    return sizeof(struct buffer);
}

static const rb_data_type_t buffer_data_type = {
    "stache-buffer",
    {
        0, // mark
        buffer_free,
        buffer_memsize
    },
    0, 0, RUBY_TYPED_FREE_IMMEDIATELY
};

static VALUE templates_init(VALUE self) {
    struct buffer *buf = calloc(1, sizeof(struct buffer));
    if (!buf) {
        rb_raise(rb_eRuntimeError, "Memory allocation failed");
    }
    buffer_init(buf);

    VALUE wrapper = TypedData_Wrap_Struct(Buffer, &buffer_data_type, buf);
    rb_ivar_set(self, id_buf, wrapper);
    return self;
}

static struct buffer *templates_get_buf(VALUE self) {
    VALUE wrapper = rb_ivar_get(self, id_buf);
    struct buffer *buf;
    TypedData_Get_Struct(wrapper, struct buffer, &buffer_data_type, buf);
    return buf;
}

void Init_stache() {
    VALUE Stache = rb_define_module("Stache");

    VALUE Templates = rb_define_class_under(Stache, "Templates", rb_cObject);
    rb_define_method(Templates, "initialize", templates_init, 0);
    rb_define_method(Templates, "render", render, 2);

    Buffer = rb_define_class_under(Stache, "Buffer", rb_cData);

    id_to_s = rb_intern("to_s");
    id_miss = rb_intern("__stache__miss__");
    id_buf = rb_intern("@buf");
}
"#;
