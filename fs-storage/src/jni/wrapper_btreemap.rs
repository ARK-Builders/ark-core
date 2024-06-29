use crate::wrapper_btreemap::WrapperBTreeMap;
use crate::file_storage::FileStorage;
// This is the interface to the JVM that we'll call the majority of our
// methods on.
use jni::JNIEnv;

// These objects are what you should use as arguments to your native
// function. They carry extra lifetime information to prevent them escaping
// this context and getting used after being GC'd.
use jni::objects::{JClass, JObject, JString, JValue};

// This is just a pointer. We'll be returning it from our function. We
// can't return one of the objects with lifetime information because the
// lifetime checker won't let us.
use jni::sys::{jboolean, jlong, jobject, jstring};
use jnix::{IntoJava, JnixEnv};

impl WrapperBTreeMap<String, String> {
    pub fn from_jlong(value: jlong) -> &'static mut Self {
        unsafe { &mut *(value as *mut WrapperBTreeMap<String, String>) }
    }
}

// JNI bindings

// #[no_mangle]
// pub extern "system" fn Java_FileStorage_create<'local>(
//     mut env: JNIEnv<'local>,
//     _class: JClass,
//     label: JString<'local>,
//     path: JString<'local>,
// ) -> jlong {
//     let label: String = env
//         .get_string(&label)
//         .expect("Couldn't get label!")
//         .into();
//     let path: String = env
//         .get_string(&path)
//         .expect("Couldn't get path!")
//         .into();

//     let file_storage: FileStorage<String, String> =
//         FileStorage::new(label, Path::new(&path)).unwrap_or_else(|err| {
//             env.throw_new("java/lang/RuntimeException", &err.to_string())
//                 .expect("Failed to throw RuntimeException");
//             FileStorage::new("".to_string(), Path::new("")).unwrap()
//         });
//     Box::into_raw(Box::new(file_storage)) as jlong
// }

#[no_mangle]
pub extern "system" fn Java_WrapperBTreeMap_create<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass,
    storage_ptr: jlong,
) -> jlong {
    // currently, only for file_storage
    let filestorage = FileStorage::from_jlong(storage_ptr);
    let wrapper = WrapperBTreeMap::new(filestorage);
    Box::into_raw(Box::new(wrapper)) as jlong
}

#[no_mangle]
pub extern "system" fn Java_WrapperBTreeMap_get<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass,
    id: JString<'local>,
    wrapper_ptr: jlong,
) -> jstring {
    let id: String = env.get_string(&id).expect("msg").into();
    let wrapper = WrapperBTreeMap::from_jlong(wrapper_ptr);
    let data: String = wrapper.get_data(id);
    env.new_string(data).unwrap().into_raw()
}

// match value {
//     Some(value) => env.new_string(value).unwrap().into_raw(),
//     None => {
//         env.throw_new(
//             "java/lang/RuntimeException",
//             &"no value present for this key".to_string(),
//         )
//         .unwrap();
//         env.new_string("").unwrap().into_raw()
//     }
// }
