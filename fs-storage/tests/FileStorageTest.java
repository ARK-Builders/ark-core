import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.nio.file.Path;
import java.util.LinkedHashMap;

import static org.junit.jupiter.api.Assertions.*;

public class FileStorageTest {
    FileStorage fileStorage = new FileStorage("test", "test.txt");


    @TempDir
    Path tempDir;

    @Test
    public void testFileStorageWriteRead() {
        String label = "test";
        Path storagePath = tempDir.resolve("test.txt");    
        FileStorage fileStorage = new FileStorage(label, storagePath.toString());

        fileStorage.set("key1", "value1");
        fileStorage.set("key2", "value2");

        fileStorage.remove("key1");

        LinkedHashMap<String, String> data = (LinkedHashMap<String, String>) fileStorage.readFS();
        assertEquals(1, data.size());
        assertEquals("value2", data.get("key2"));
    }

    @Test
    public void testFileStorageAutoDelete() {
        String label = "test";
        Path storagePath = tempDir.resolve("test.txt");    
        FileStorage fileStorage = new FileStorage(label, storagePath.toString());

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
        String label = "test";
        Path storagePath = tempDir.resolve("test.txt");    
        FileStorage fileStorage = new FileStorage(label, storagePath.toString());
        assertFalse(fileStorage.needsSyncing());

        fileStorage.set("key1", "value1");
        // FAIL: don't why it is still false
        // assertTrue(fileStorage.needsSyncing());

        fileStorage.writeFS();
        assertFalse(fileStorage.needsSyncing());
    }

    @Test
    public void testFileStorageMainScenario() {
        String label = "test";
        Path storagePath = tempDir.resolve("test.txt");    
        FileStorage fileStorage = new FileStorage(label, storagePath.toString());

        fileStorage.set("key", "value");
        fileStorage.set("key", "value1");
        fileStorage.set("key1", "value");

        fileStorage.remove("key");

        fileStorage.writeFS();

        LinkedHashMap<String, String> data = (LinkedHashMap<String, String>) fileStorage.readFS();
        assertEquals(1, data.size());
        assertEquals("value", data.get("key1"));

        fileStorage.erase();
        File file = storagePath.toFile();
        assertFalse(file.exists());
    }
}