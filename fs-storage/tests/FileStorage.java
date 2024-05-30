public class FileStorage {
    private long fileStoragePtr;

    static {
        System.loadLibrary("fs_storage");
    }

    private static native long create(String label, String path);

    private static native void set(String id, String value, long file_storage_ptr);

    private static native void remove(String id, long file_storage_ptr);

    private static native boolean needsSyncing(long file_storage_ptr);

    private static native Object readFS(long file_storage_ptr);

    private static native void writeFS(long file_storage_ptr);

    private static native void erase(long file_storage_ptr);

    public FileStorage(String label, String path) {
        this.fileStoragePtr = create(label, path);
    }

    public void set(String id, String value) {
        set(id, value, this.fileStoragePtr);
    }

    public void remove(String id) {
        remove(id, this.fileStoragePtr);
    }

    public boolean needsSyncing() {
        return needsSyncing(this.fileStoragePtr);
    }

    public Object readFS() {
        return readFS(this.fileStoragePtr);
    }

    public void writeFS() {
        writeFS(this.fileStoragePtr);
    }

    public void erase() {
        erase(this.fileStoragePtr);
    }

    public static void main(String[] args) {
        FileStorage fileStorage = new FileStorage("test", "test.txt");
        System.out.println(fileStorage.fileStoragePtr);
        fileStorage.set("key", "value");
        fileStorage.set("key", "value1");
        fileStorage.set("key1", "value");
        fileStorage.remove("key");
        System.out.println(fileStorage.needsSyncing());
        fileStorage.writeFS();
        System.out.println(fileStorage.needsSyncing());
        System.out.println(fileStorage.readFS());
        fileStorage.erase();
    }
}
