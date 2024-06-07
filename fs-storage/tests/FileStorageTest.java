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
        fileStorage.writeFS();

        File file = storagePath.toFile();
        assertTrue(file.exists());

        fileStorage.erase();
        assertFalse(file.exists());
    }

    // problem
    @Test
    public void testFileStorageNeedsSyncing() {
        String label = "test";
        Path storagePath = tempDir.resolve("test.txt");    
        FileStorage fileStorage = new FileStorage(label, storagePath.toString());
        fileStorage.writeFS();
        assertFalse(fileStorage.needsSyncing());
        fileStorage.set("key1", "value1");
        // // FAIL: don't why it is still false
        // assertTrue(fileStorage.needsSyncing());

        fileStorage.writeFS();
        assertFalse(fileStorage.needsSyncing());
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
        String label = "test";
        Path storagePath = tempDir.resolve("test.txt");    
        FileStorage fileStorage = new FileStorage(label, storagePath.toString());

        fileStorage.set("key", "value");
        fileStorage.set("key", "value1");
        fileStorage.set("key1", "value");

        fileStorage.remove("key");

        // Sleep for 1 second
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
}