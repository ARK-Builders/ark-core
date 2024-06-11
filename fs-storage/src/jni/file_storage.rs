use std::collections::BTreeMap;
use std::path::Path;

use jni::signature::ReturnType;
// This is the interface to the JVM that we'll call the majority of our
// methods on.
use jni::JNIEnv;

// These objects are what you should use as arguments to your native
// function. They carry extra lifetime information to prevent them escaping
// this context and getting used after being GC'd.
use jni::objects::{JClass, JString, JValue};

// This is just a pointer. We'll be returning it from our function. We
// can't return one of the objects with lifetime information because the
// lifetime checker won't let us.
use jni::sys::{jboolean, jlong, jobject};

use crate::base_storage::BaseStorage;

use crate::file_storage::FileStorage;

impl FileStorage<String, String> {
    fn from_jlong<'a>(value: jlong) -> &'a mut Self {
        unsafe { &mut *(value as *mut FileStorage<String, String>) }
    }
}

#[no_mangle]
pub extern "system" fn Java_FileStorage_create<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass,
    label: JString<'local>,
    path: JString<'local>,
) -> jlong {
    let label: String = env
        .get_string(&label)
        .expect("Couldn't get label!")
        .into();
    let path: String = env
        .get_string(&path)
        .expect("Couldn't get path!")
        .into();

    let file_storage: FileStorage<String, String> =
        FileStorage::new(label, Path::new(&path));
    Box::into_raw(Box::new(file_storage)) as jlong
}

#[no_mangle]
pub extern "system" fn Java_FileStorage_set<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass,
    id: JString<'local>,
    value: JString<'local>,
    file_storage_ptr: jlong,
) {
    let id: String = env.get_string(&id).expect("msg").into();
    let value: String = env.get_string(&value).expect("msg").into();

    FileStorage::from_jlong(file_storage_ptr).set(id, value);
}

#[no_mangle]
pub extern "system" fn Java_FileStorage_remove<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass,
    id: JString<'local>,
    file_storage_ptr: jlong,
) {
    let id: String = env.get_string(&id).unwrap().into();
    FileStorage::from_jlong(file_storage_ptr)
        .remove(&id)
        .unwrap_or_else(|err| {
            let error_class = env
                .find_class("java/lang/RuntimeException")
                .unwrap();
            env.throw_new(error_class, &err.to_string())
                .unwrap();
        });
}

#[no_mangle]
pub extern "system" fn Java_FileStorage_needsSyncing(
    mut env: JNIEnv<'_>,
    _class: JClass,
    file_storage_ptr: jlong,
) -> jboolean {
    match FileStorage::from_jlong(file_storage_ptr).needs_syncing() {
        Ok(updated) => updated as jboolean,
        Err(err) => {
            let error_class = env
                .find_class("java/lang/RuntimeException")
                .unwrap();
            env.throw_new(error_class, &err.to_string())
                .unwrap();
            false as jboolean
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_FileStorage_readFS(
    mut env: JNIEnv<'_>,
    _class: JClass,
    file_storage_ptr: jlong,
) -> jobject {
    let data: BTreeMap<String, String> =
        FileStorage::from_jlong(file_storage_ptr)
            .read_fs()
            .expect("not able to read data");

    // Create a new LinkedHashMap object
    let linked_hash_map_class =
        env.find_class("java/util/LinkedHashMap").unwrap();
    let linked_hash_map = env
        .new_object(linked_hash_map_class, "()V", &[])
        .unwrap();

    // Get the put method ID
    let put_method_id = env
        .get_method_id(
            "java/util/LinkedHashMap",
            "put",
            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        )
        .unwrap();

    // Insert each key-value pair from the BTreeMap into the LinkedHashMap
    for (key, value) in data {
        let j_key = env.new_string(key).unwrap();
        let j_value = env.new_string(value).unwrap();
        let j_key = JValue::from(&j_key).as_jni();
        let j_value = JValue::from(&j_value).as_jni();
        unsafe {
            env.call_method_unchecked(
                &linked_hash_map,
                put_method_id,
                ReturnType::Object,
                &[j_key, j_value],
            )
            .unwrap()
        };
    }

    // Return the LinkedHashMap as a raw pointer
    linked_hash_map.as_raw()
}

#[no_mangle]
pub extern "system" fn Java_FileStorage_writeFS(
    mut env: JNIEnv<'_>,
    _class: JClass,
    file_storage_ptr: jlong,
) {
    FileStorage::from_jlong(file_storage_ptr)
        .write_fs()
        .unwrap_or_else(|err| {
            let error_class = env
                .find_class("java/lang/RuntimeException")
                .unwrap();
            env.throw_new(error_class, &err.to_string())
                .unwrap();
        });
}

#[allow(clippy::suspicious_doc_comments)]
///! Safety: The FileStorage instance is dropped after this call
#[no_mangle]
pub extern "system" fn Java_FileStorage_erase(
    mut env: JNIEnv<'_>,
    _class: JClass,
    file_storage_ptr: jlong,
) {
    let file_storage = unsafe {
        Box::from_raw(file_storage_ptr as *mut FileStorage<String, String>)
    };
    file_storage.erase().unwrap_or_else(|err| {
        let error_class = env
            .find_class("java/lang/RuntimeException")
            .unwrap();
        env.throw_new(error_class, &err.to_string())
            .unwrap();
    });
}

#[no_mangle]
pub extern "system" fn Java_FileStorage_merge(
    mut env: JNIEnv<'_>,
    _class: JClass,
    file_storage_ptr: jlong,
    other_file_storage_ptr: jlong,
) {
    FileStorage::from_jlong(file_storage_ptr)
        .merge_from(FileStorage::from_jlong(other_file_storage_ptr))
        .unwrap_or_else(|err| {
            let error_class = env
                .find_class("java/lang/RuntimeException")
                .unwrap();
            env.throw_new(error_class, &err.to_string())
                .unwrap();
        });
}
