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

static VALUE cCGI;

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

static VALUE context_fetch(VALUE stack, const char *key) {
    for (long i = RARRAY_LEN(stack) - 1; i >= 0; i--) {
        VALUE context = RARRAY_AREF(stack, i);
        VALUE value = fetch(context, key);
        if (value != Qundef) {
            return value;
        }
    }
    return Qundef;
}

static VALUE fetch_path(VALUE stack, const struct path *path) {
    VALUE value = context_fetch(stack, path->keys[0]);
    for (long i = 1; i < path->length; i++) {
        value = fetch(value, path->keys[i]);
    }
    return value;
}

static void append_value(VALUE buf, VALUE stack, const struct path *path, bool escape) {
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
    rb_str_buf_append(buf, escape ? optimized_escape_html(value) : value);
}

static void section(VALUE buf, VALUE stack, const struct path *path, void (*block)(VALUE, VALUE)) {
    VALUE value = fetch_path(stack, path);
    switch (rb_type(value)) {
        case T_ARRAY:
            for (long i = 0; i < RARRAY_LEN(value); i++) {
                rb_ary_push(stack, RARRAY_AREF(value, i));
                block(buf, stack);
                rb_ary_pop(stack);
            }
            break;
        case T_NIL:
        case T_UNDEF:
        case T_FALSE:
            break;
        case T_TRUE:
            block(buf, stack);
            break;
        default:
            rb_ary_push(stack, value);
            block(buf, stack);
            rb_ary_pop(stack);
            break;
    }
}

static void inverted(VALUE buf, VALUE stack, const struct path *path, void (*block)(VALUE, VALUE)) {
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
static void initialize();

void Init_stache() {
    VALUE Stache = rb_define_module("Stache");
    VALUE Templates = rb_define_class_under(Stache, "Templates", rb_cObject);
    rb_define_singleton_method(Templates, "render", render, 2);

    rb_require("cgi");
    id_to_s = rb_intern("to_s");
    id_miss = rb_intern("__stache__miss__");
    initialize();
}
"#;
