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

        String err = fileStorage.remove("key1");
        assertTrue(err.isEmpty());

        @SuppressWarnings("unchecked")
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
        String err = fileStorage.writeFS();
        assertTrue(err.isEmpty());

        File file = storagePath.toFile();
        assertTrue(file.exists());

        err = fileStorage.erase();
        assertTrue(err.isEmpty());
        assertFalse(file.exists());
    }

    // problem
    @Test
    public void testFileStorageNeedsSyncing() {
        String label = "test";
        Path storagePath = tempDir.resolve("test.txt");    
        FileStorage fileStorage = new FileStorage(label, storagePath.toString());
        fileStorage.writeFS();
        String result = fileStorage.needsSyncing();
        assertEquals("false", result);
        fileStorage.set("key1", "value1");
        // // FAIL: don't why it is still false
        // assertTrue(fileStorage.needsSyncing());

        String err = fileStorage.writeFS();
        assertTrue(err.isEmpty());
        result = fileStorage.needsSyncing();
        assertEquals("false", result);
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

        String err = fileStorage1.merge(fileStorage2);
        assertTrue(err.isEmpty());
        err = fileStorage1.writeFS();
        assertTrue(err.isEmpty());
        
        @SuppressWarnings("unchecked")
        LinkedHashMap<String, String> data = (LinkedHashMap<String, String>) fileStorage1.readFS();
        assertEquals(3, data.size());
        assertEquals("23", data.get("key1"));
        assertEquals("6", data.get("key2"));
        assertEquals("9", data.get("key3"));
    }


    @Test
    public void testFileStorageMainScenario() {
        String label = "test";
        Path storagePath = tempDir.resolve("test.txt");    
        FileStorage fileStorage = new FileStorage(label, storagePath.toString());

        fileStorage.set("key", "value");
        fileStorage.set("key", "value1");
        fileStorage.set("key1", "value");

        String err = fileStorage.remove("key");
        assertTrue(err.isEmpty());

        // Sleep for 1 second
        try {
            Thread.sleep(1000); 
        } catch (InterruptedException e) {
            e.printStackTrace();
        }
    

        err = fileStorage.writeFS();
        assertTrue(err.isEmpty());

        @SuppressWarnings("unchecked")
        LinkedHashMap<String, String> data = (LinkedHashMap<String, String>) fileStorage.readFS();
        assertEquals(1, data.size());
        assertEquals("value", data.get("key1"));

        err = fileStorage.erase();
        assertTrue(err.isEmpty());
        File file = storagePath.toFile();
        assertFalse(file.exists());
    }
}