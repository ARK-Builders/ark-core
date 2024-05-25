use crate::index::{discover_paths, IndexEntry};
use crate::ResourceIndex;
use canonical_path::CanonicalPathBuf;
use dev_hash::Crc32;
use fs_atomic_versions::initialize;
use std::fs::File;
#[cfg(target_os = "linux")]
use std::fs::Permissions;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;

use std::path::PathBuf;
use std::time::SystemTime;
use uuid::Uuid;

const FILE_SIZE_1: u64 = 10;
const FILE_SIZE_2: u64 = 11;

const FILE_NAME_1: &str = "test1.txt";
const FILE_NAME_2: &str = "test2.txt";
const FILE_NAME_3: &str = "test3.txt";

const CRC32_1: Crc32 = Crc32(3817498742);
const CRC32_2: Crc32 = Crc32(1804055020);

fn get_temp_dir() -> PathBuf {
    create_dir_at(std::env::temp_dir())
}

fn create_dir_at(path: PathBuf) -> PathBuf {
    let mut dir_path = path.clone();
    dir_path.push(Uuid::new_v4().to_string());
    std::fs::create_dir(&dir_path).expect("Could not create temp dir");
    dir_path
}

fn create_file_at(
    path: PathBuf,
    size: Option<u64>,
    name: Option<&str>,
) -> (File, PathBuf) {
    let mut file_path = path.clone();
    if let Some(file_name) = name {
        file_path.push(file_name);
    } else {
        file_path.push(Uuid::new_v4().to_string());
    }
    let file =
        File::create(file_path.clone()).expect("Could not create temp file");
    file.set_len(size.unwrap_or(0))
        .expect("Could not set file size");
    (file, file_path)
}

fn run_test_and_clean_up(test: impl FnOnce(PathBuf) + std::panic::UnwindSafe) {
    initialize();

    let path = get_temp_dir();
    let result = std::panic::catch_unwind(|| test(path.clone()));
    std::fs::remove_dir_all(path.clone())
        .expect("Could not clean up after test");
    if result.is_err() {
        panic!("{}", result.err().map(|_| "Test panicked").unwrap())
    }
    assert!(result.is_ok());
}

// resource index build

#[test]
fn index_build_should_process_1_file_successfully() {
    run_test_and_clean_up(|path| {
        create_file_at(path.clone(), Some(FILE_SIZE_1), None);

        let actual: ResourceIndex<Crc32> = ResourceIndex::build(path.clone());

        assert_eq!(actual.root, path.clone());
        assert_eq!(actual.path2id.len(), 1);
        assert_eq!(actual.id2path.len(), 1);
        assert!(actual.id2path.contains_key(&CRC32_1));
        assert_eq!(actual.collisions.len(), 0);
        assert_eq!(actual.size(), 1);
    })
}

#[test]
fn index_build_should_process_colliding_files_correctly() {
    run_test_and_clean_up(|path| {
        create_file_at(path.clone(), Some(FILE_SIZE_1), None);
        create_file_at(path.clone(), Some(FILE_SIZE_1), None);

        let actual: ResourceIndex<Crc32> = ResourceIndex::build(path.clone());

        assert_eq!(actual.root, path.clone());
        assert_eq!(actual.path2id.len(), 2);
        assert_eq!(actual.id2path.len(), 1);
        assert!(actual.id2path.contains_key(&CRC32_1));
        assert_eq!(actual.collisions.len(), 1);
        assert_eq!(actual.size(), 2);
    })
}

// resource index update

#[test]
fn update_all_should_handle_renamed_file_correctly() {
    run_test_and_clean_up(|path| {
        create_file_at(path.clone(), Some(FILE_SIZE_1), Some(FILE_NAME_1));
        create_file_at(path.clone(), Some(FILE_SIZE_2), Some(FILE_NAME_2));

        let mut actual: ResourceIndex<Crc32> =
            ResourceIndex::build(path.clone());

        assert_eq!(actual.collisions.len(), 0);
        assert_eq!(actual.size(), 2);

        // rename test2.txt to test3.txt
        let mut name_from = path.clone();
        name_from.push(FILE_NAME_2);
        let mut name_to = path.clone();
        name_to.push(FILE_NAME_3);
        std::fs::rename(name_from, name_to)
            .expect("Should rename file successfully");

        let update = actual
            .update_all()
            .expect("Should update index correctly");

        assert_eq!(actual.collisions.len(), 0);
        assert_eq!(actual.size(), 2);
        assert_eq!(update.deleted.len(), 1);
        assert_eq!(update.added.len(), 1);
    })
}

#[test]
fn update_all_should_index_new_file_successfully() {
    run_test_and_clean_up(|path| {
        create_file_at(path.clone(), Some(FILE_SIZE_1), None);

        let mut actual: ResourceIndex<Crc32> =
            ResourceIndex::build(path.clone());

        let (_, expected_path) =
            create_file_at(path.clone(), Some(FILE_SIZE_2), None);

        let update = actual
            .update_all()
            .expect("Should update index correctly");

        assert_eq!(actual.root, path.clone());
        assert_eq!(actual.path2id.len(), 2);
        assert_eq!(actual.id2path.len(), 2);
        assert!(actual.id2path.contains_key(&CRC32_1));
        assert!(actual.id2path.contains_key(&CRC32_2));
        assert_eq!(actual.collisions.len(), 0);
        assert_eq!(actual.size(), 2);
        assert_eq!(update.deleted.len(), 0);
        assert_eq!(update.added.len(), 1);

        let added_key = CanonicalPathBuf::canonicalize(expected_path.clone())
            .expect("CanonicalPathBuf should be fine");
        assert_eq!(
            update
                .added
                .get(&added_key)
                .expect("Key exists")
                .clone(),
            CRC32_2
        )
    })
}

#[test]
fn index_new_should_index_new_file_successfully() {
    run_test_and_clean_up(|path| {
        create_file_at(path.clone(), Some(FILE_SIZE_1), None);
        let mut index: ResourceIndex<Crc32> =
            ResourceIndex::build(path.clone());

        let (_, new_path) =
            create_file_at(path.clone(), Some(FILE_SIZE_2), None);

        let update = index
            .index_new(&new_path)
            .expect("Should update index correctly");

        assert_eq!(index.root, path.clone());
        assert_eq!(index.path2id.len(), 2);
        assert_eq!(index.id2path.len(), 2);
        assert!(index.id2path.contains_key(&CRC32_1));
        assert!(index.id2path.contains_key(&CRC32_2));
        assert_eq!(index.collisions.len(), 0);
        assert_eq!(index.size(), 2);
        assert_eq!(update.deleted.len(), 0);
        assert_eq!(update.added.len(), 1);

        let added_key = CanonicalPathBuf::canonicalize(new_path.clone())
            .expect("CanonicalPathBuf should be fine");
        assert_eq!(
            update
                .added
                .get(&added_key)
                .expect("Key exists")
                .clone(),
            CRC32_2
        )
    })
}

#[test]
fn update_one_should_error_on_new_file() {
    run_test_and_clean_up(|path| {
        create_file_at(path.clone(), Some(FILE_SIZE_1), None);
        let mut index = ResourceIndex::build(path.clone());

        let (_, new_path) =
            create_file_at(path.clone(), Some(FILE_SIZE_2), None);

        let update = index.update_one(&new_path, CRC32_2);

        assert!(update.is_err())
    })
}

#[test]
fn update_one_should_index_delete_file_successfully() {
    run_test_and_clean_up(|path| {
        create_file_at(path.clone(), Some(FILE_SIZE_1), Some(FILE_NAME_1));

        let mut actual = ResourceIndex::build(path.clone());

        let mut file_path = path.clone();
        file_path.push(FILE_NAME_1);
        std::fs::remove_file(file_path.clone())
            .expect("Should remove file successfully");

        let update = actual
            .update_one(&file_path.clone(), CRC32_1)
            .expect("Should update index successfully");

        assert_eq!(actual.root, path.clone());
        assert_eq!(actual.path2id.len(), 0);
        assert_eq!(actual.id2path.len(), 0);
        assert_eq!(actual.collisions.len(), 0);
        assert_eq!(actual.size(), 0);
        assert_eq!(update.deleted.len(), 1);
        assert_eq!(update.added.len(), 0);

        assert!(update.deleted.contains(&CRC32_1))
    })
}

#[test]
fn update_all_should_error_on_files_without_permissions() {
    run_test_and_clean_up(|path| {
        create_file_at(path.clone(), Some(FILE_SIZE_1), Some(FILE_NAME_1));
        let (file, _) =
            create_file_at(path.clone(), Some(FILE_SIZE_2), Some(FILE_NAME_2));

        let mut actual: ResourceIndex<Crc32> =
            ResourceIndex::build(path.clone());

        assert_eq!(actual.collisions.len(), 0);
        assert_eq!(actual.size(), 2);
        #[cfg(target_os = "linux")]
        file.set_permissions(Permissions::from_mode(0o222))
            .expect("Should be fine");

        let update = actual
            .update_all()
            .expect("Should update index correctly");

        assert_eq!(actual.collisions.len(), 0);
        assert_eq!(actual.size(), 2);
        assert_eq!(update.deleted.len(), 0);
        assert_eq!(update.added.len(), 0);
    })
}

// error cases

#[test]
fn update_one_should_not_update_absent_path() {
    run_test_and_clean_up(|path| {
        let mut missing_path = path.clone();
        missing_path.push("missing/directory");
        let mut actual = ResourceIndex::build(path.clone());
        let old_id = Crc32(2);
        let result = actual
            .update_one(&missing_path, old_id.clone())
            .map(|i| i.deleted.clone().take(&old_id))
            .ok()
            .flatten();

        assert_eq!(result, Some(Crc32(2)));
    })
}

#[test]
fn update_one_should_index_new_path() {
    run_test_and_clean_up(|path| {
        let mut missing_path = path.clone();
        missing_path.push("missing/directory");
        let mut actual = ResourceIndex::build(path.clone());
        let old_id = Crc32(2);
        let result = actual
            .update_one(&missing_path, old_id.clone())
            .map(|i| i.deleted.clone().take(&old_id))
            .ok()
            .flatten();

        assert_eq!(result, Some(Crc32(2)));
    })
}

#[test]
fn should_not_index_empty_file() {
    run_test_and_clean_up(|path| {
        create_file_at(path.clone(), Some(0), None);
        let actual: ResourceIndex<Crc32> = ResourceIndex::build(path.clone());

        assert_eq!(actual.root, path.clone());
        assert_eq!(actual.path2id.len(), 0);
        assert_eq!(actual.id2path.len(), 0);
        assert_eq!(actual.collisions.len(), 0);
    })
}

#[test]
fn should_not_index_hidden_file() {
    run_test_and_clean_up(|path| {
        create_file_at(path.clone(), Some(FILE_SIZE_1), Some(".hidden"));
        let actual: ResourceIndex<Crc32> = ResourceIndex::build(path.clone());

        assert_eq!(actual.root, path.clone());
        assert_eq!(actual.path2id.len(), 0);
        assert_eq!(actual.id2path.len(), 0);
        assert_eq!(actual.collisions.len(), 0);
    })
}

#[test]
fn should_not_index_1_empty_directory() {
    run_test_and_clean_up(|path| {
        create_dir_at(path.clone());

        let actual: ResourceIndex<Crc32> = ResourceIndex::build(path.clone());

        assert_eq!(actual.root, path.clone());
        assert_eq!(actual.path2id.len(), 0);
        assert_eq!(actual.id2path.len(), 0);
        assert_eq!(actual.collisions.len(), 0);
    })
}

#[test]
fn discover_paths_should_not_walk_on_invalid_path() {
    run_test_and_clean_up(|path| {
        let mut missing_path = path.clone();
        missing_path.push("missing/directory");
        let actual = discover_paths(missing_path);
        assert_eq!(actual.len(), 0);
    })
}

#[test]
fn index_entry_order() {
    let old1 = IndexEntry {
        id: Crc32(2),
        modified: SystemTime::UNIX_EPOCH,
    };
    let old2 = IndexEntry {
        id: Crc32(1),
        modified: SystemTime::UNIX_EPOCH,
    };

    let new1 = IndexEntry {
        id: Crc32(1),
        modified: SystemTime::now(),
    };
    let new2 = IndexEntry {
        id: Crc32(2),
        modified: SystemTime::now(),
    };

    assert_eq!(new1, new1);
    assert_eq!(new2, new2);
    assert_eq!(old1, old1);
    assert_eq!(old2, old2);

    assert_ne!(new1, new2);
    assert_ne!(new1, old1);

    assert!(new1 > old1);
    assert!(new1 > old2);
    assert!(new2 > old1);
    assert!(new2 > old2);
    assert!(new2 > new1);
}

/// Test the performance of `ResourceIndex::build` on a specific directory.
///
/// This test evaluates the performance of building a resource
/// index using the `ResourceIndex::build` method on a given directory.
/// It measures the time taken to build the resource index and prints the
/// number of collisions detected.
#[test]
fn test_build_resource_index() {
    use std::time::Instant;

    let path = "../test-assets/"; // The path to the directory to index
    assert!(
        std::path::Path::new(path).is_dir(),
        "The provided path is not a directory or does not exist"
    );

    let start_time = Instant::now();
    let index: ResourceIndex<Crc32> = ResourceIndex::build(path.to_string());
    let elapsed_time = start_time.elapsed();

    println!("Number of paths: {}", index.id2path.len());
    println!("Number of resources: {}", index.id2path.len());
    println!("Number of collisions: {}", index.collisions.len());
    println!("Time taken: {:?}", elapsed_time);
}
