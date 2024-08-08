//! This module provides tests for the `ResourceIndex` functionality using
//! different hash algorithms.
//!
//! The tests are parameterized by various hash types, such as `Blake3` and
//! `Crc32`, to ensure the implementation works consistently across different
//! hashing algorithms.
//!
//! # Structure
//!
//! - **Macros**:
//!  - `for_each_type!`: A macro that takes a list of hash function types and a
//!    block of code to execute for each hash type.
//!
//! - **Test Functions**:
//!   - Defined to test various aspects of `ResourceIndex`, parameterized by
//!     hash type.
//!
//! - **Helper Functions**:
//!   - `get_indexed_resource_from_file`: Helper to create `IndexedResource`
//!     from a file path.
//!
//! # Usage
//!
//! To add a new test for a specific hash type:
//! 1. Write a block of code generic over the hash type (a Type implementing
//!    ResourceId trait).
//! 2. Use the `for_each_type!` macro to execute the block of code for each
//!    desired hash type.

use dev_hash::{Blake3, Crc32};
use std::{fs, path::Path};

use anyhow::{anyhow, Result};
use tempfile::TempDir;

use data_resource::ResourceId;

use crate::{
    index::IndexedResource, utils::load_or_build_index, ResourceIndex,
};

/// A macro that takes a list of hash function types and a block of code to
/// execute for each hash type.
#[macro_export]
macro_rules! for_each_type {
    ($($hash_type:ty),+ => $body:block) => {
        $(
            {
                type Id = $hash_type;
                $body
            }
        )+
    };
}

/// A helper function to get [`IndexedResource`] from a file path
fn get_indexed_resource_from_file<Id: ResourceId, P: AsRef<Path>>(
    path: P,
    parent_dir: P,
) -> Result<IndexedResource<Id>> {
    let id = Id::from_path(&path)?;

    let relative_path = path
        .as_ref()
        .strip_prefix(parent_dir)
        .map_err(|_| anyhow!("Failed to get relative path"))?;

    Ok(IndexedResource::new(
        id,
        relative_path.to_path_buf(),
        fs::metadata(path)?.modified()?,
    ))
}

/// Test storing and loading the resource index.
///
/// ## Test scenario:
/// - Build a resource index in the temporary directory.
/// - Store the index.
/// - Load the stored index.
/// - Assert that the loaded index matches the original index.
#[test]
fn test_store_and_load_index() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_store_and_load_index")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file_path = root_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        let index: ResourceIndex<Id> =
            ResourceIndex::build(root_path).expect("Failed to build index");
        assert_eq!(index.len(), 1, "{:?}", index);
        index.store().expect("Failed to store index");

        let loaded_index =
            load_or_build_index(root_path, false).expect("Failed to load index");

        assert_eq!(index, loaded_index, "{:?} != {:?}", index, loaded_index);
    });
}

/// Test storing and loading the resource index with collisions.
///
/// ## Test scenario:
/// - Build a resource index in the temporary directory.
/// - Write duplicate files with the same content.
/// - Store the index.
/// - Load the stored index.
/// - Assert that the loaded index matches the original index.
#[test]
fn test_store_and_load_index_with_collisions() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir =
            TempDir::with_prefix("ark_test_store_and_load_index_with_collisions")
                .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file_path = root_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        let file_path2 = root_path.join("file2.txt");
        fs::write(&file_path2, "file content").expect("Failed to write to file");

        let file_path3 = root_path.join("file3.txt");
        fs::write(&file_path3, "file content").expect("Failed to write to file");

        let file_path4 = root_path.join("file4.txt");
        fs::write(&file_path4, "file content").expect("Failed to write to file");

        // Now we have 4 files with the same content (same checksum)

        let index: ResourceIndex<Id> =
            ResourceIndex::build(root_path).expect("Failed to build index");
        let checksum = Id::from_path(&file_path).expect("Failed to get checksum");
        assert_eq!(index.len(), 4, "{:?}", index);
        assert_eq!(index.collisions().len(), 1, "{:?}", index);
        assert_eq!(index.collisions()[&checksum].len(), 4, "{:?}", index);
        index.store().expect("Failed to store index");

        let loaded_index =
            load_or_build_index(root_path, false).expect("Failed to load index");

        assert_eq!(index, loaded_index, "{:?} != {:?}", index, loaded_index);
    });
}

/// Test building an index with a file.
///
/// ## Test scenario:
/// - Create a file within the temporary directory.
/// - Build a resource index in the temporary directory.
/// - Assert that the index contains one entry.
/// - Assert that the resource retrieved by path matches the expected resource.
/// - Assert that the resource retrieved by ID matches the expected resource.
#[test]
fn test_build_index_with_file() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_build_index_with_file")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file_path = root_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");
        let expected_resource: IndexedResource<Id> =
            get_indexed_resource_from_file(&file_path, &root_path.to_path_buf())
                .expect("Failed to get indexed resource");

        let index = ResourceIndex::build(root_path).expect("Failed to build index");
        assert_eq!(index.len(), 1, "{:?}", index);

        let resource = index
            .get_resource_by_path("file.txt")
            .expect("Failed to get resource");
        assert_eq!(
            resource, expected_resource,
            "{:?} != {:?}",
            resource, expected_resource
        );
    });
}

/// Test building an index with an empty file.
///
/// ## Test scenario:
/// - Create an empty file within the temporary directory.
/// - Create a file with content within the temporary directory.
/// - Build a resource index in the temporary directory.
/// - Assert that the index contains one entries.
#[test]
fn test_build_index_with_empty_file() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_build_index_with_empty_file")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let empty_file_path = root_path.join("empty_file.txt");
        fs::write(&empty_file_path, "").expect("Failed to write to file");

        let file_path = root_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        let index: ResourceIndex<Id> =
            ResourceIndex::build(root_path).expect("Failed to build index");
        assert_eq!(index.len(), 1, "{:?}", index);
    });
}

/// Test building an index with a directory.
///
/// ## Test scenario:
/// - Create a subdirectory within the temporary directory.
/// - Create a file within the subdirectory.
/// - Build a resource index in the temporary directory.
/// - Assert that the index contains one entry.
/// - Assert that the resource retrieved by path matches the expected resource.
/// - Assert that the resource retrieved by ID matches the expected resource.
#[test]
fn test_build_index_with_directory() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_build_index_with_directory")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        // print path
        println!("Root path: {:?}", root_path);
        // assert it exists
        assert!(root_path.exists(), "Root path does not exist");

        let dir_path = root_path.join("dir");
        fs::create_dir(&dir_path).expect("Failed to create dir");

        // print dir path
        println!("Dir path: {:?}", dir_path);
        // assert it exists
        assert!(dir_path.exists(), "Dir path does not exist");

        let file_path = dir_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        // print file path
        println!("File path: {:?}", file_path);
        // assert it exists
        assert!(file_path.exists(), "File path does not exist");

        let expected_resource: IndexedResource<Id> =
            get_indexed_resource_from_file(&file_path, &root_path.to_path_buf())
                .expect("Failed to get indexed resource");

        // print expected resource
        println!("Expected resource: {:?}", expected_resource);

        let index = ResourceIndex::build(root_path).expect("Failed to build index");
        assert_eq!(index.len(), 1, "{:?}", index);

        let resource = index
            .get_resource_by_path("dir/file.txt")
            .expect("Failed to get resource");
        assert_eq!(
            resource, expected_resource,
        );
    });
}

/// Test building an index with multiple files.
///
/// ## Test scenario:
/// - Create multiple files within the temporary directory.
/// - Build a resource index in the temporary directory.
/// - Assert that the index contains two entries.
/// - Assert that the resource retrieved by path for each file matches the
///   expected resource.
#[test]
fn test_build_index_with_multiple_files() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir =
            TempDir::with_prefix("ark_test_build_index_with_multiple_files")
                .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file1_path = root_path.join("file1.txt");
        fs::write(&file1_path, "file1 content").expect("Failed to write to file");
        let file2_path = root_path.join("file2.txt");
        fs::write(&file2_path, "file2 content").expect("Failed to write to file");

        let expected_resource1: IndexedResource<Id> =
            get_indexed_resource_from_file(&file1_path, &root_path.to_path_buf())
                .expect("Failed to get indexed resource");
        let expected_resource2 =
            get_indexed_resource_from_file(&file2_path, &root_path.to_path_buf())
                .expect("Failed to get indexed resource");

        let index = ResourceIndex::build(root_path).expect("Failed to build index");
        assert_eq!(index.len(), 2, "{:?}", index);

        let resource = index
            .get_resource_by_path("file1.txt")
            .expect("Failed to get resource");
        assert_eq!(resource, expected_resource1, "{:?}", resource);

        let resource = index
            .get_resource_by_path("file2.txt")
            .expect("Failed to get resource");
        assert_eq!(resource, expected_resource2, "{:?}", resource);
    });
}

/// Test building an index with multiple directories.
///
/// ## Test scenario:
/// - Create multiple directories within the temporary directory, each
///   containing a file.
/// - Build a resource index in the temporary directory.
/// - Assert that the index contains two entries.
/// - Assert that the resources retrieved by path for each file match the
///   expected resources.
#[test]
fn test_build_index_with_multiple_directories() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir =
            TempDir::with_prefix("ark_test_build_index_with_multiple_directories")
                .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let dir1_path = root_path.join("dir1");
        fs::create_dir(&dir1_path).expect("Failed to create dir");
        let file1_path = dir1_path.join("file1.txt");
        fs::write(&file1_path, "file1 content").expect("Failed to write to file");

        let dir2_path = root_path.join("dir2");
        fs::create_dir(&dir2_path).expect("Failed to create dir");
        let file2_path = dir2_path.join("file2.txt");
        fs::write(&file2_path, "file2 content").expect("Failed to write to file");

        let expected_resource1: IndexedResource<Id> =
            get_indexed_resource_from_file(&file1_path, &root_path.to_path_buf())
                .expect("Failed to get indexed resource");
        let expected_resource2 =
            get_indexed_resource_from_file(&file2_path, &root_path.to_path_buf())
                .expect("Failed to get indexed resource");

        let index = ResourceIndex::build(root_path).expect("Failed to build index");
        assert_eq!(index.len(), 2, "{:?}", index);

        let resource = index
            .get_resource_by_path("dir1/file1.txt")
            .expect("Resource not found");
        assert_eq!(resource, expected_resource1, "{:?}", resource);

        let resource = index
            .get_resource_by_path("dir2/file2.txt")
            .expect("Resource not found");
        assert_eq!(resource, expected_resource2, "{:?}", resource);
    });
}

/// Test updating the resource index.
///
/// ## Test scenario:
/// - Create files within the temporary directory.
/// - Build a resource index in the temporary directory.
/// - Assert that the index initially contains the expected number of entries.
/// - Create a new file, modify an existing file, and remove another file.
/// - Update the resource index.
/// - Assert that the index contains the expected number of entries after the
///   update.
/// - Assert that the entries in the index match the expected state after the
///   update.
#[test]
fn test_resource_index_update() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_resource_index_update")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file_path = root_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        let image_path = root_path.join("image.png");
        fs::write(&image_path, "image content").expect("Failed to write to file");

        let mut index: ResourceIndex<Id> =
            ResourceIndex::build(root_path).expect("Failed to build index");
        std::thread::sleep(std::time::Duration::from_secs(1));
        index.store().expect("Failed to store index");
        assert_eq!(index.len(), 2, "{:?}", index);

        // create new file
        let new_file_path = root_path.join("new_file.txt");
        fs::write(&new_file_path, "new file content")
            .expect("Failed to write to file");

        // modify file
        fs::write(&file_path, "updated file content")
            .expect("Failed to write to file");

        // remove file
        fs::remove_file(&image_path).expect("Failed to remove file");

        index
            .update_all()
            .expect("Failed to update index");
        // Index now contains 2 resources (file.txt and new_file.txt)
        assert_eq!(index.len(), 2, "{:?}", index);

        let resource = index
            .get_resource_by_path("file.txt")
            .expect("Resource not found");
        let expected_resource =
            get_indexed_resource_from_file(&file_path, &root_path.to_path_buf())
                .expect("Failed to get indexed resource");
        assert_eq!(resource, expected_resource, "{:?}", resource);

        let _resource = index
            .get_resource_by_path("new_file.txt")
            .expect("Resource not found");

        assert!(
            index.get_resource_by_path("image.png").is_none(),
            "{:?}",
            index
        );
    });
}

/// Test adding colliding files to the index.
///
/// ## Test scenario:
/// - Create a file within the temporary directory.
/// - Build a resource index in the temporary directory.
/// - Assert that the index initially contains the expected number of entries.
/// - Create a new file with the same checksum as the existing file.
/// - Track the addition of the new file in the index.
/// - Assert that the index contains the expected number of entries after the
///   addition.
/// - Assert index.collisions contains the expected number of entries.
#[test]
fn test_add_colliding_files() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_add_colliding_files")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file_path = root_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        let mut index: ResourceIndex<Id> =
            ResourceIndex::build(root_path).expect("Failed to build index");
        index.store().expect("Failed to store index");
        assert_eq!(index.len(), 1, "{:?}", index);

        let new_file_path = root_path.join("new_file.txt");
        fs::write(&new_file_path, "file content").expect("Failed to write to file");

        index
            .update_all()
            .expect("Failed to update index");

        assert_eq!(index.len(), 2, "{:?}", index);
        assert_eq!(index.collisions().len(), 1, "{:?}", index);
    });
}

/// Test `ResourceIndex::num_collisions()` method.
///
/// ## Test scenario:
/// - Create a file within the temporary directory.
/// - Build a resource index in the temporary directory.
/// - Assert that the index initially contains the expected number of entries.
/// - Create 2 new files with the same checksum as the existing file.
/// - Update the index.
/// - Assert that the index contains the expected number of entries after the
///   update.
#[test]
fn test_num_collisions() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_num_collisions")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file_path = root_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        let mut index: ResourceIndex<Id> =
            ResourceIndex::build(root_path).expect("Failed to build index");
        index.store().expect("Failed to store index");
        assert_eq!(index.len(), 1, "{:?}", index);

        let new_file_path = root_path.join("new_file.txt");
        fs::write(&new_file_path, "file content").expect("Failed to write to file");

        let new_file_path2 = root_path.join("new_file2.txt");
        fs::write(&new_file_path2, "file content")
            .expect("Failed to write to file");

        index
            .update_all()
            .expect("Failed to update index");

        assert_eq!(index.len(), 3, "{:?}", index);
        assert_eq!(index.num_collisions(), 3, "{:?}", index);
    });
}

/// Test that we don't index hidden files.
///
/// ## Test scenario:
/// - Create a hidden file within the temporary directory.
/// - Build a resource index in the temporary directory.
/// - Assert that the index initially contains the expected number of entries.
///   (0)
#[test]
fn test_hidden_files() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_hidden_files")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file_path = root_path.join(".hidden_file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        let index: ResourceIndex<Id> =
            ResourceIndex::build(root_path).expect("Failed to build index");
        index.store().expect("Failed to store index");
        assert_eq!(index.len(), 0, "{:?}", index);
    });
}

/// Test that we detect added files in `update_all`.
///
/// ## Test scenario:
/// - Create a file within the temporary directory.
/// - Build a resource index in the temporary directory.
/// - Create a new file.
/// - Update the resource index.
/// - Assert that the return from `update_all` is that `added` includes the
///  new file.
#[test]
fn test_update_all_added_files() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_added_files")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file_path = root_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        let mut index: ResourceIndex<Id> =
            ResourceIndex::build(root_path).expect("Failed to build index");

        let new_file_path = root_path.join("new_file.txt");
        fs::write(&new_file_path, "new file content")
            .expect("Failed to write to file");

        let update_result = index.update_all().expect("Failed to update index");
        assert_eq!(update_result.added().len(), 1, "{:?}", update_result);
    });
}

/// Test that we detect updated files using the last modified time.
///
/// ## Test scenario:
/// - Create a file within the temporary directory.
/// - Build a resource index in the temporary directory.
/// - Sleep for a second to ensure the last modified time is different.
/// - Update the file.
/// - Update the resource index.
/// - Assert that the return from `update_all` is that `added` includes the
///  updated file.
#[test]
fn test_update_all_updated_files() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_updated_files")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file_path = root_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        let mut index: ResourceIndex<Id> =
            ResourceIndex::build(root_path).expect("Failed to build index");

        std::thread::sleep(std::time::Duration::from_secs(1));

        fs::write(&file_path, "updated file content")
            .expect("Failed to write to file");

        let update_result = index.update_all().expect("Failed to update index");
        assert_eq!(update_result.added().len(), 1, "{:?}", update_result);
    });
}

/// Test that we detect deleted files in `update_all`.
///
/// ## Test scenario:
/// - Create a file within the temporary directory.
/// - Build a resource index in the temporary directory.
/// - Remove the file.
/// - Update the resource index.
/// - Assert that the return from `update_all` is that `removed` includes the
///   deleted file.
#[test]
fn test_update_all_deleted_files() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_deleted_files")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file_path = root_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        let mut index: ResourceIndex<Id> =
            ResourceIndex::build(root_path).expect("Failed to build index");

        fs::remove_file(&file_path).expect("Failed to remove file");

        let update_result = index.update_all().expect("Failed to update index");
        assert_eq!(update_result.removed().len(), 1, "{:?}", update_result);
    });
}

/// Test that we detect files with the same hash but different content in
/// `update_all`.
///
/// ## Test scenario:
/// - Create a file within the temporary directory.
/// - Build a resource index in the temporary directory.
/// - Modify the file.
/// - Create a new file with the same content but different name (path).
/// - Update the resource index.
/// - Assert that the return from `update_all` is that `added` includes both
///   files.
#[test]
fn test_update_all_files_with_same_hash() {
    for_each_type!(Crc32, Blake3 => {
        let temp_dir = TempDir::with_prefix("ark_test_files_with_same_hash")
            .expect("Failed to create temp dir");
        let root_path = temp_dir.path();

        let file_path = root_path.join("file.txt");
        fs::write(&file_path, "file content").expect("Failed to write to file");

        let mut index: ResourceIndex<Id> =
            ResourceIndex::build(root_path).expect("Failed to build index");

        std::thread::sleep(std::time::Duration::from_secs(1));

        fs::write(&file_path, "updated file content")
            .expect("Failed to write to file");

        let new_file_path = root_path.join("new_file.txt");
        fs::write(&new_file_path, "updated file content")
            .expect("Failed to write to file");

        let update_result = index.update_all().expect("Failed to update index");
        // The lentgh of `added` should be 1 because the new file has the same
        // content as the updated file.
        assert_eq!(update_result.added().len(), 1, "{:?}", update_result);

        // The length of `added`'s first element should be 2
        assert_eq!(update_result.added().values().next().unwrap().len(), 2);

        // The length of `collisions` should be 1 because the new file has the
        // same content as the updated file.
        assert_eq!(index.collisions().len(), 1, "{:?}", index);
    });
}
