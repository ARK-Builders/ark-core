/// Represents the synchronization status of the storage.
public enum SyncStatus {
    /// No synchronization needed.
    InSync,
    /// In-memory key-value mapping is stale.
    MappingStale,
    /// External file system storage is stale.
    StorageStale,
    /// In-memory key-value mapping and external file system storage diverge.
    Diverge, 
}
