/**
 * file_storage
 */
public class FileStorage {
    private static native long create(String label, String path);

    private static native void set(String id, String value, long file_storage_ptr);

    private static native void remove(String id, long file_storage_ptr);

    private static native boolean needs_syncing(long file_storage_ptr);

    private static native Object read_fs(long file_storage_ptr);

    private static native void write_fs(long file_storage_ptr);

    private static native void erase(long file_storage_ptr);

    static {
        System.loadLibrary("fs_storage");
    }

    public static void main(String[] args) {
        long file_storage_ptr = create("test", "test.txt");
        System.out.println(file_storage_ptr);
        set("key", "value", file_storage_ptr);
        set("key", "value1", file_storage_ptr);
        set("key1", "value", file_storage_ptr);
        remove("key", file_storage_ptr);
        System.out.println(needs_syncing(file_storage_ptr));
        write_fs(file_storage_ptr);
        System.out.println(needs_syncing(file_storage_ptr));
        System.out.println(read_fs(file_storage_ptr));
        erase(file_storage_ptr);
    }
}
