package dev.arkbuilders.core;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.nio.file.Path;
import java.util.LinkedHashMap;

import static org.junit.jupiter.api.Assertions.*;
import static org.junit.jupiter.api.Assertions.assertThrows;

public class FileStorageTest {
    FileStorage fileStorage = new FileStorage("test", "test.txt");

    @TempDir
    Path tempDir;

    @Test
    public void testFileStorageWriteRead() {
        Path storagePath = tempDir.resolve("test.txt");
        FileStorage fileStorage = new FileStorage("test", storagePath.toString());

        fileStorage.set("key1", "value1");
        fileStorage.set("key2", "value2");

        fileStorage.remove("key1");
        fileStorage.writeFS();

        @SuppressWarnings("unchecked")
        LinkedHashMap<String, String> data = (LinkedHashMap<String, String>) fileStorage.readFS();
        assertEquals(1, data.size());
        assertEquals("value2", data.get("key2"));
    }

    @Test
    public void testFileStorageAutoDelete() {
        Path storagePath = tempDir.resolve("test.txt");
        FileStorage fileStorage = new FileStorage("test", storagePath.toString());

        fileStorage.set("key1", "value1");
        fileStorage.set("key1", "value2");
        fileStorage.writeFS();

        File file = storagePath.toFile();
        assertTrue(file.exists());

        fileStorage.erase();
        assertFalse(file.exists());
    }

    @Test
    public void testFileStorageNeedsSyncing() {
        Path storagePath = tempDir.resolve("test.txt");
        FileStorage fileStorage = new FileStorage("test", storagePath.toString());
        fileStorage.writeFS();
        assertEquals(FileStorage.SyncStatus.InSync, fileStorage.syncStatus());
        fileStorage.set("key1", "value1");
        assertEquals(FileStorage.SyncStatus.StorageStale, fileStorage.syncStatus());
        fileStorage.writeFS();
        assertEquals(FileStorage.SyncStatus.InSync, fileStorage.syncStatus());
    }

    @Test
    public void testFileStorageMonoidCombine() {
        Path storagePath1 = tempDir.resolve("test1.txt");
        Path storagePath2 = tempDir.resolve("test2.txt");
        FileStorage fileStorage1 = new FileStorage("test1", storagePath1.toString());
        FileStorage fileStorage2 = new FileStorage("test2", storagePath2.toString());

        fileStorage1.set("key1", "2");
        fileStorage1.set("key2", "6");

        fileStorage2.set("key1", "3");
        fileStorage2.set("key3", "9");

        fileStorage1.merge(fileStorage2);
        fileStorage1.writeFS();

        @SuppressWarnings("unchecked")
        LinkedHashMap<String, String> data = (LinkedHashMap<String, String>) fileStorage1.readFS();
        assertEquals(3, data.size());
        assertEquals("23", data.get("key1"));
        assertEquals("6", data.get("key2"));
        assertEquals("9", data.get("key3"));
    }

    @Test
    public void testFileStorageMainScenario() {
        Path storagePath = tempDir.resolve("test.txt");
        FileStorage fileStorage = new FileStorage("test", storagePath.toString());

        fileStorage.set("key", "value");
        fileStorage.set("key", "value1");
        fileStorage.set("key1", "value");

        fileStorage.remove("key");

        try {
            Thread.sleep(1000);
        } catch (InterruptedException e) {
            e.printStackTrace();
        }

        fileStorage.writeFS();

        @SuppressWarnings("unchecked")
        LinkedHashMap<String, String> data = (LinkedHashMap<String, String>) fileStorage.readFS();
        assertEquals(1, data.size());
        assertEquals("value", data.get("key1"));

        fileStorage.erase();
        File file = storagePath.toFile();
        assertFalse(file.exists());
    }

    @Test
    public void testRemoveException() {
        Path storagePath = tempDir.resolve("test.txt");
        FileStorage fileStorage = new FileStorage("test", storagePath.toString());
        Exception exception = assertThrows(RuntimeException.class, () -> fileStorage.remove("invalid_id"));
        assertTrue(exception.getMessage().matches("Storage error.*"));
    }

    @Test
    public void testSyncException() {
        Path storagePath = tempDir.resolve("test.txt");
        FileStorage fileStorage = new FileStorage("test", storagePath.toString());
        Exception exception = assertThrows(RuntimeException.class, () -> fileStorage.sync());
        assertTrue(exception.getMessage().matches("IO error.*"));
    }

    @Test
    public void testCreateException() {
        Path storagePath = tempDir.resolve("");
        Exception exception = assertThrows(RuntimeException.class, () -> new FileStorage("", storagePath.toString()));
        assertTrue(exception.getMessage().matches("IO error.*"));
    }

    @Test
    public void testEraseException() {
        Path storagePath = tempDir.resolve("test.txt");
        FileStorage fileStorage = new FileStorage("test", storagePath.toString());
        Exception exception = assertThrows(RuntimeException.class, () -> fileStorage.erase());
        assertTrue(exception.getMessage().matches("Storage error.*"));
    }

    @Test
    public void testReadException() {
        Path storagePath = tempDir.resolve("test.txt");
        FileStorage fileStorage = new FileStorage("test", storagePath.toString());
        Exception exception = assertThrows(RuntimeException.class, () -> fileStorage.readFS());
        assertTrue(exception.getMessage().matches("Storage error.*"));
    }
}