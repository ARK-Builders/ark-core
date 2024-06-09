public class FileStorage {
    private long fileStoragePtr;

    static {
        System.loadLibrary("fs_storage");
    }

    private static native long create(String label, String path);

    private static native void set(String id, String value, long file_storage_ptr);

    private static native String remove(String id, long file_storage_ptr);

    private static native String needsSyncing(long file_storage_ptr);

    private static native Object readFS(long file_storage_ptr);

    private static native String writeFS(long file_storage_ptr);

    private static native String erase(long file_storage_ptr);

    private static native String merge(long file_storage_ptr, long other_file_storage_ptr);

    public FileStorage(String label, String path) {
        this.fileStoragePtr = create(label, path);
    }

    public void set(String id, String value) {
        set(id, value, this.fileStoragePtr);
    }

    public String remove(String id) {
        return remove(id, this.fileStoragePtr);
    }

    public String needsSyncing() {
        return needsSyncing(this.fileStoragePtr);
    }

    public Object readFS() {
        return readFS(this.fileStoragePtr);
    }

    public String writeFS() {
        return writeFS(this.fileStoragePtr);
    }

    public String erase() {
        return erase(this.fileStoragePtr);
    }

    public String merge(FileStorage other) {
        return merge(this.fileStoragePtr, other.fileStoragePtr);
    }
}
