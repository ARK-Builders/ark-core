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

    private static native void merge(long file_storage_ptr, long other_file_storage_ptr);

    public FileStorage(String label, String path) {
        this.fileStoragePtr = create(label, path);
    }

    public void set(String id, String value) {
        set(id, value, this.fileStoragePtr);
    }

    public void remove(String id) {
        try {
            remove(id, this.fileStoragePtr);
        } catch (RuntimeException e) {
            System.err.println("Error removing file storage: " + e.getMessage());
        }
    }

    public boolean needsSyncing() {
        try {
            return needsSyncing(this.fileStoragePtr);
        } catch (RuntimeException e) {
            System.err.println("Error checking if file storage needs syncing: " + e.getMessage());
            return false;
        }
    }

    public Object readFS() {
        try {
            return readFS(this.fileStoragePtr);
        } catch (RuntimeException e) {
            System.err.println("Error reading file storage: " + e.getMessage());
            return null;
        }
    }

    public void writeFS() {
        try {
            writeFS(this.fileStoragePtr);
        } catch (RuntimeException e) {
            System.err.println("Error writing file storage: " + e.getMessage());
        }
    }

    public void erase() {
        try {
            erase(this.fileStoragePtr);
        } catch (RuntimeException e) {
            System.err.println("Error erasing file storage: " + e.getMessage());
        }
    }

    public void merge(FileStorage other) {
        try {
            merge(this.fileStoragePtr, other.fileStoragePtr);
        } catch (RuntimeException e) {
            System.err.println("Error merging file storage: " + e.getMessage());
        }
    }
}
