pub const RUNTIME: &'static str = r#"
#include "ruby.h"
#include <stdbool.h>
#include <string.h>

static const char *DOT = ".";

static ID id_escape_html;
static ID id_key_p;
static ID id_to_s;

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
            VALUE key_str = rb_str_new_cstr(key);
            if (RTEST(rb_funcall(context, id_key_p, 1, key_str))) {
                return rb_hash_aref(context, key_str);
            } else {
                VALUE sym = ID2SYM(rb_intern(key));
                if (RTEST(rb_funcall(context, id_key_p, 1, sym))) {
                    return rb_hash_aref(context, sym);
                } else {
                    return Qundef;
                }
            }
        }
        case T_STRUCT: {
            VALUE sym = ID2SYM(rb_intern(key));
            VALUE members = rb_struct_members(context);
            if (RTEST(rb_ary_includes(members, sym))) {
                return rb_struct_aref(context, sym);
            } else {
                return Qundef;
            }
        }
        case T_OBJECT: {
            ID method = rb_intern(key);
            if (rb_respond_to(context, method) && rb_obj_method_arity(context, method) == 0) {
                return rb_funcall(context, method, 0);
            } else {
                return Qundef;
            }
        }
        default:
            return Qundef;
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
    return Qnil;
}

static VALUE fetch_path(VALUE stack, const struct path *path) {
    VALUE value = context_fetch(stack, path->keys[0]);
    if (value == Qnil) {
        return Qnil;
    }
    for (long i = 1; i < path->length; i++) {
        value = fetch(value, path->keys[i]);
        if (value == Qundef) {
            return Qnil;
        }
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
    rb_str_buf_append(buf, escape ? rb_funcall(cCGI, id_escape_html, 1, value) : value);
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
        case T_HASH:
        case T_OBJECT:
        case T_STRUCT:
            rb_ary_push(stack, value);
            block(buf, stack);
            rb_ary_pop(stack);
            break;
        case T_NIL:
        case T_UNDEF:
        case T_FALSE:
            break;
        default:
            block(buf, stack);
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

void Init_stache() {
    VALUE Stache = rb_define_module("Stache");
    VALUE Templates = rb_define_class_under(Stache, "Templates", rb_cObject);
    rb_define_singleton_method(Templates, "render", render, 2);

    rb_require("cgi");
    cCGI = rb_const_get(rb_cObject, rb_intern("CGI"));
    id_escape_html = rb_intern("escapeHTML");
    id_key_p = rb_intern("key?");
    id_to_s = rb_intern("to_s");
}
"#;
