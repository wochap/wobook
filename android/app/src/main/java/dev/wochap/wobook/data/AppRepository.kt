package dev.wochap.wobook.data

import dev.wochap.wobook.ffi.AddRequest
import dev.wochap.wobook.ffi.AddResult
import dev.wochap.wobook.ffi.AppListener
import dev.wochap.wobook.ffi.Bookmark
import dev.wochap.wobook.ffi.DataChange
import dev.wochap.wobook.ffi.Device
import dev.wochap.wobook.ffi.DeviceView
import dev.wochap.wobook.ffi.Hit
import dev.wochap.wobook.ffi.ImportReport
import dev.wochap.wobook.ffi.InterchangeFormat
import dev.wochap.wobook.ffi.ListQuery
import dev.wochap.wobook.ffi.Metadata
import dev.wochap.wobook.ffi.PairingEvent
import dev.wochap.wobook.ffi.PairingOffer
import dev.wochap.wobook.ffi.SearchQuery
import dev.wochap.wobook.ffi.SyncStatus
import dev.wochap.wobook.ffi.TagCount
import dev.wochap.wobook.ffi.UpdateRequest
import dev.wochap.wobook.ffi.WobookApp
import dev.wochap.wobook.ffi.WobookException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Thin facade over the Rust `WobookApp` (design D3). The Rust read model is
 * the cache: Kotlin only keeps a revision counter, the sync status, devices
 * and pairing events, refreshed from `AppListener` callbacks.
 */
class AppRepository(
    cacheDir: File,
    private val opener: suspend (AppListener) -> WobookApp,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val lock = Mutex()
    @Volatile private var app: WobookApp? = null

    /** Site icons, shared by Home rows and the refresh worker. */
    val favicons = FaviconCache(File(cacheDir, "favicons")) { origin -> io { it.fetchFavicon(origin) } }

    private val _revision = MutableStateFlow(0L)
    /** Bumped on every local or remote document change. */
    val revision: StateFlow<Long> = _revision.asStateFlow()

    private val _sync = MutableStateFlow<SyncStatus?>(null)
    val syncStatus: StateFlow<SyncStatus?> = _sync.asStateFlow()

    private val _devices = MutableStateFlow<List<DeviceView>>(emptyList())
    val devices: StateFlow<List<DeviceView>> = _devices.asStateFlow()

    private val _pairing = MutableSharedFlow<PairingEvent>(extraBufferCapacity = 32)
    val pairingEvents: SharedFlow<PairingEvent> = _pairing.asSharedFlow()

    private val _openError = MutableStateFlow<String?>(null)
    val openError: StateFlow<String?> = _openError.asStateFlow()

    private val listener = object : AppListener {
        override fun onDataChanged(change: DataChange) {
            _revision.value = _revision.value + 1
            scope.launch { refreshDevices() }
        }

        override fun onSyncStatus(status: SyncStatus) {
            _sync.value = status
            scope.launch { refreshDevices() }
        }

        override fun onPairingEvent(event: PairingEvent) {
            _pairing.tryEmit(event)
            if (event is PairingEvent.Completed) scope.launch { refreshDevices() }
        }
    }

    /** Opens the Rust core once per process. */
    suspend fun app(): WobookApp = app ?: lock.withLock {
        app ?: withContext(Dispatchers.IO) {
            try {
                opener(listener).also {
                    app = it
                    _openError.value = null
                    _sync.value = it.syncStatus()
                    _devices.value = it.devices()
                }
            } catch (e: Exception) {
                _openError.value = e.userMessage()
                throw e
            }
        }
    }

    /** Opened instance or null (UI that must not trigger an open). */
    fun current(): WobookApp? = app

    private suspend fun <T> io(block: suspend (WobookApp) -> T): T =
        withContext(Dispatchers.IO) { block(app()) }

    suspend fun refreshDevices() {
        runCatching { _devices.value = io { it.devices() } }
        runCatching { _sync.value = io { it.syncStatus() } }
    }

    // bookmarks
    suspend fun search(query: String, tags: List<String>, limit: Int? = null): List<Hit> =
        io { it.search(SearchQuery(query, tags, false, limit?.toUInt())) }

    suspend fun list(tags: List<String> = emptyList()): List<Bookmark> =
        io { it.list(ListQuery(tags, false, null)) }

    suspend fun get(url: String): Bookmark? = io { it.get(url) }
    suspend fun tags(): List<TagCount> = io { it.tags() }
    suspend fun librarySize(): Long = io { it.librarySize().toLong() }

    suspend fun add(
        url: String, title: String?, description: String?, tags: List<String>,
        fetch: Boolean, merge: Boolean,
    ): AddResult = io { it.add(AddRequest(url, title, description, tags, fetch, merge)) }

    suspend fun update(request: UpdateRequest): Bookmark = io { it.update(request) }
    suspend fun rename(from: String, to: String): Bookmark = io { it.rename(from, to) }
    suspend fun delete(url: String): Bookmark = io { it.delete(url) }
    suspend fun restore(url: String): Bookmark = io { it.restore(url) }
    suspend fun fetchMetadata(url: String): Metadata = io { it.fetchMetadata(url) }
    suspend fun import(format: InterchangeFormat, path: String): ImportReport = io { it.import(format, path) }
    suspend fun export(format: InterchangeFormat, path: String): Long =
        io { it.export(format, path, false).toLong() }

    // devices and sync
    suspend fun thisDevice(): Device = io { it.thisDevice() }
    suspend fun setDeviceName(name: String): String = io { it.setDeviceName(name) }
    suspend fun renameDevice(id: String, name: String) { io { it.renameDevice(id, name) }; refreshDevices() }
    suspend fun revokeDevice(id: String) { io { it.revokeDevice(id) }; refreshDevices() }
    suspend fun syncNow() = io { it.syncNow() }
    suspend fun setForeground(on: Boolean) = io { it.setForeground(on) }

    // pairing
    suspend fun startPairingOffer(): PairingOffer = io { it.startPairingOffer() }
    suspend fun joinPairing(payload: String): String = io { it.joinPairing(payload) }
    suspend fun confirmPairing(id: String, trust: Boolean) = io { it.confirmPairing(id, trust) }
    suspend fun pendingConfirmations() = io { it.pendingConfirmations() }
}

/** UniFFI renders messages as `message=…`; strip that for the UI. */
fun Throwable.userMessage(): String {
    val raw = when (this) {
        is WobookException.InvalidUrl -> this.reason
        is WobookException.InvalidRequest -> this.reason
        is WobookException.NotFound -> this.reason
        is WobookException.Exists -> this.reason
        is WobookException.Io -> this.reason
        is WobookException.IdentityLost -> this.reason
        is WobookException.Pairing -> this.reason
        is WobookException.UnknownDevice -> this.reason
        is WobookException.Closed -> this.reason
        is WobookException.Internal -> this.reason
        else -> message ?: javaClass.simpleName
    }
    return raw.removePrefix("reason=")
}
