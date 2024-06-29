import java.util.Iterator;
import java.util.Map;
import java.util.NoSuchElementException;

public class FileStorageIterator implements Iterator<Map.Entry<String, String>>, AutoCloseable {
    private long fileStorageIteratorPtr;

    private static native boolean hasNext(long file_storage_iterator_ptr);

    private static native Object next(long file_storage_iterator_ptr);

    private static native void destroyIterator(long file_storage_iterator_ptr);

    FileStorageIterator(long fileStorageIteratorPtr) {
        this.fileStorageIteratorPtr = fileStorageIteratorPtr;
    }

    @Override
    public boolean hasNext() {
        return hasNext(this.fileStorageIteratorPtr);
    }

    @SuppressWarnings("unchecked")
    @Override
    public Map.Entry<String, String> next() {
        if (!hasNext()) {
            throw new NoSuchElementException();
        }
        return (Map.Entry<String, String>) next(this.fileStorageIteratorPtr);
    }

    @Override
    public void close() {
        if (this.fileStorageIteratorPtr != 0) {
            destroyIterator(this.fileStorageIteratorPtr);
            this.fileStorageIteratorPtr = 0;
        }
    }
}
