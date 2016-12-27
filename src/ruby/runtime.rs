pub const RUNTIME: &'static str = r#"
#include "ruby.h"
#include <stdbool.h>

static VALUE escape_html(VALUE value) {
    VALUE cgi = rb_const_get(rb_cObject, rb_intern("CGI"));
    return rb_funcall(cgi, rb_intern("escapeHTML"), 1, value);
}

static VALUE fetch(VALUE context, VALUE key) {
    switch (rb_type(context)) {
        case T_HASH:
            return rb_hash_aref(context, key);
        case T_STRUCT:
            // TODO Check rb_struct_members to avoid name error.
            return rb_struct_aref(context, key);
        case T_OBJECT:
            // TODO Prevent undefined method error and check arity.
            return rb_funcall(context, rb_to_id(key), 0);
        default:
            return Qundef;
    }
}

static VALUE context_fetch(VALUE stack, VALUE key) {
    for (long i = RARRAY_LEN(stack) - 1; i >= 0; i--) {
        VALUE context = rb_ary_entry(stack, i);
        VALUE value = fetch(context, key);
        if (value != Qundef) {
            return value;
        }
    }
    return Qnil;
}

static VALUE fetch_path(VALUE stack, VALUE path) {
    VALUE value = context_fetch(stack, rb_ary_entry(path, 0));
    if (value == Qnil) {
        return Qnil;
    }
    for (long i = 1; i < RARRAY_LEN(path); i++) {
        value = fetch(value, rb_ary_entry(path, i));
        if (value == Qundef) {
            return Qnil;
        }
    }
    return value;
}

static void append_value(VALUE buf, VALUE stack, VALUE path, bool escape) {
    VALUE value = fetch_path(stack, path);
    switch (rb_type(value)) {
        case T_NIL:
        case T_UNDEF:
            return;
        case T_STRING:
            break;
        default:
            value = rb_funcall(value, rb_intern("to_s"), 0);
            break;
    }
    rb_str_buf_append(buf, escape ? escape_html(value) : value);
}

static void section(VALUE buf, VALUE stack, VALUE path, void (*block)(VALUE, VALUE)) {
    VALUE value = fetch_path(stack, path);
    switch (rb_type(value)) {
        case T_ARRAY:
            for (long i = 0; i < RARRAY_LEN(value); i++) {
                rb_ary_push(stack, rb_ary_entry(value, i));
                block(buf, stack);
                rb_ary_pop(stack);
            }
            break;
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

static void inverted(VALUE buf, VALUE stack, VALUE path, void (*block)(VALUE, VALUE)) {
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
"#;
