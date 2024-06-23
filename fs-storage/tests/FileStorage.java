public class FileStorage {
    private long fileStoragePtr;

    static {
        System.loadLibrary("fs_storage");
    }

    private static native long create(String label, String path);

    private static native void set(String id, String value, long file_storage_ptr);

    private static native void remove(String id, long file_storage_ptr);

    private static native void sync(long file_storage_ptr);

    private static native SyncStatus syncStatus(long file_storage_ptr);

    private static native Object readFS(long file_storage_ptr);

    private static native void writeFS(long file_storage_ptr);

    private static native void erase(long file_storage_ptr);

    private static native void merge(long file_storage_ptr, long other_file_storage_ptr);

    public FileStorage(String label, String path) {
        this.fileStoragePtr = create(label, path);
    }

    public void set(String id, String value) {
        set(id, value, this.fileStoragePtr);
    }

    public void remove(String id) {
        remove(id, this.fileStoragePtr);
    }

    public void sync() {
        sync(this.fileStoragePtr);
    }

    public SyncStatus syncStatus() {
        return syncStatus(this.fileStoragePtr);
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

    public void merge(FileStorage other) {
        merge(this.fileStoragePtr, other.fileStoragePtr);
    }
}
