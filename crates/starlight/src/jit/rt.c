//! Small runtime for Starlight's JIT. This file includes main functions that is
//! used when starlight bytecode is transpiled to C. Some of constants here is
//! passed by JIT itself (i.e `JSOBJECT_TYPEID`) so we do not invoke *two*
//! functions to check types of objects.
#include <inttypes.h>
#include <stdio.h>
typedef union {
  float f;
  uint32_t bits;
} f32bits;

typedef union {
  double f;
  uint64_t bits;
} f64bits;

#define f32_to_bits(fval) (((f32bits){.f = fval}).bits)
#define f32_from_bits(fval) (((f32bits){.bits = fval}).f)
#define f64_to_bits(fval) (((f64bits){.f = fval}).bits)
#define f64_from_bits(fval) (((f64bits){.bits = fval}).f)

typedef uint64_t jsval;
#define FIRST_TAG 0xfff9ull
#define LAST_TAG 0xffffull
#define EMPTY_INVALID_TAG FIRST_TAG
#define UNDEFINED_NULL_TAG (FIRST_TAG + 1)
#define BOOL_TAG (FIRST_TAG + 2)
#define INT32_TAG (FIRST_TAG + 3)
#define NATIVE_VALUE_TAG (FIRST_TAG + 4)
#define STR_TAG (FIRST_TAG + 5)
#define OBJECT_TAG (FIRST_TAG + 6)
#define FIRST_PTR_TAG STR_TAG

typedef enum {
  ExtEmpty = EMPTY_INVALID_TAG * 2 + 1,
  ExtUndefined = UNDEFINED_NULL_TAG * 2,
  ExtNull = UNDEFINED_NULL_TAG * 2 + 1,
  ExtBool = BOOL_TAG * 2,
  ExtInt32 = INT32_TAG * 2,
  ExtNative1 = NATIVE_VALUE_TAG * 2,
  ExtNative2 = NATIVE_VALUE_TAG * 2 + 1,
  ExtStr1 = STR_TAG * 2,
  ExtStr2 = STR_TAG * 2 + 1,
  ExtObject1 = OBJECT_TAG * 2,
  ExtObject2 = OBJECT_TAG * 2 + 1,
} ExtendedTag;

#define NUM_TAG_EXP_BITS (uint64_t)16
#define NUM_DATA_BITS (64 - NUM_TAG_EXP_BITS)
#define TAG_WIDTH 4
#define TAG_MASK ((1 << TAG_WIDTH) - 1)
#define DATA_MASK (((uint64_t)1 << (uint64_t)NUM_DATA_BITS) - (uint64_t)1)
#define ETAG_WIDTH 5
#define ETAG_MASK ((1 << ETAG_WIDTH) - 1)
typedef struct {
  jsval value;
  uint8_t isErr;
} result;

typedef struct gcheader_s {
  size_t vtable;
  uint8_t cell_state;
  uint32_t size;
  uint8_t pad;
  uint8_t pad1;
  uint8_t pad2;
} gcheader;

typedef struct variable_s {
  jsval value;
  uint8_t mutable;
} variable;

typedef struct environment_s {
  gcheader *parent;
  variable *values_ptr;
  uint32_t values_count;
} environment;

typedef struct callframe_s {
  struct callframe_s *prev;
  jsval *sp;
  jsval *limit;
  jsval callee;
  uint8_t *ip;
  gcheader *code_block;
  jsval this;
  uint8_t ctor;
  uint8_t exit_on_return;
  gcheader *env;

} callframe;

extern uint64_t get_jscell_type_id(void *x);
extern result jsval_to_number_slow(void *rt, jsval val);
extern result op_add_slow(void *, jsval, jsval);
extern result op_sub_slow(void *, jsval, jsval);
extern result op_div_slow(void *, jsval, jsval);
extern result op_mul_slow(void *, jsval, jsval);
extern result op_rem_slow(void *, jsval, jsval);
extern result op_shl_slow(void *, jsval, jsval);
extern result op_shr_slow(void *, jsval, jsval);
extern result op_ushr_slow(void *, jsval, jsval);
extern result op_less_slow(void *, jsval, jsval);
extern result op_lesseq_slow(void *, jsval, jsval);
extern result op_greater_slow(void *, jsval, jsval);
extern result op_greatereq_slow(void *, jsval, jsval);
#define jsval_from_raw(x) (uint64_t) x
#define jsval_get_tag(val) (uint32_t)(val >> NUM_DATA_BITS)
#define jsval_get_etag(val) (uint32_t)((val >> (NUM_DATA_BITS - 1)))
#define jsval_combine_tags(a, b) ((a & TAG_MASK) << TAG_WIDTH) | (b & TAG_MASK)
#define jsval_new(val, tag) (jsval)(val | ((uint64_t)tag << NUM_DATA_BITS))
#define jsval_new_ext(val, tag) (jsval)(val | (tag << (NUM_DATA_BITS - 1)))
#define jsval_new_object(val) jsval_new((uint64_t)val, OBJECT_TAG)
#define jsval_new_bool(x) jsval_new(x, BOOL_TAG)
#define jsval_new_null() jsval_new_ext(0, ExtNull)
#define jsval_new_int32(x) jsval_new((uint64_t)(uint32_t)(int32_t)x, INT32_TAG)
#define jsval_new_undef(x) jsval_new_ext(0, ExtUndefined)
#define jsval_new_f64(x) f64_to_bits(x)
#define jsval_new_nan(x) 0x7ff8000000000000ull
#define jsval_new_untrusted_f64(x) (x != x ? jsval_new_nan() : jsval_new_f64(x))
#define jsval_is_null(x) jsval_get_etag(x) == ExtNull
#define jsval_is_undef(x) jsval_get_etag(x) == ExtUndefined
#define jsval_is_empty(x) jsval_get_etag(x) == ExtEmpty
#define jsval_is_int32(x) jsval_get_tag(x) == INT32_TAG
#define jsval_is_bool(x) jsval_get_tag(x) == BOOL_TAG
#define jsval_is_object(x) jsval_get_tag(x) == OBJECT_TAG
#define jsval_is_double(x) x < ((FIRST_TAG) << NUM_DATA_BITS)
#define jsval_get_int32(x) ((int32_t)x)
#define jsval_get_double(x) f64_from_bits(x)
#define jsval_get_bool(x) (x & 0x1)
#define jsval_get_object(x) (void *)(x & DATA_MASK)
#define jsval_get_number(x)                                                    \
  (jsval_is_int32(x) ? (double)(jsval_get_int32(x)) : jsval_get_double(x))

#define jsval_is_number(x) jsval_is_int32(x) || jsval_is_double(x)
#define jsval_is_jsobject(x)                                                   \
  (jsval_is_object(x)                                                          \
       ? get_jscell_type_id(jsval_get_object(x)) == JSOBJECT_TYPEID            \
       : false)
#define jsval_is_jsstring(x)                                                   \
  (jsval_is_object(x)                                                          \
       ? get_jscell_type_id(jsval_get_object(x)) == JSSTRING_TYPEID            \
       : false)

#define result_ok(val) ((result){.isErr = 0, .value = val})

/// Return value converted to double number or error if exception happened.
result jsval_to_number(void *rt, jsval val) {
  if (jsval_is_int32(val)) {
    return result_ok(jsval_new_f64(jsval_get_number(val)));
  }
  if (jsval_is_double(val)) {
    return result_ok(jsval_new_f64(jsval_get_double(val)));
  }
  if (jsval_is_null(val)) {
    return result_ok(jsval_new_f64(0.0));
  }
  if (jsval_is_undef(val)) {
    return result_ok(jsval_new_nan());
  }
  if (jsval_is_bool(val)) {
    return result_ok((jsval_get_bool(val) ? 1.0 : 0.0));
  }
  return jsval_to_number_slow(rt, val);
}
int main() {
  jsval val = jsval_new_int32(42);
  printf("%f\n", jsval_get_number(val));
}