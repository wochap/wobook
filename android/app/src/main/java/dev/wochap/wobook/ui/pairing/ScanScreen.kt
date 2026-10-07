package dev.wochap.wobook.ui.pairing

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import android.util.Size
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.adamglin.PhosphorIcons
import com.adamglin.phosphoricons.Regular
import com.adamglin.phosphoricons.regular.CameraSlash
import com.adamglin.phosphoricons.regular.ClipboardText
import com.adamglin.phosphoricons.regular.Flashlight
import com.adamglin.phosphoricons.regular.X
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.data.userMessage
import dev.wochap.wobook.domain.QrPayload
import dev.wochap.wobook.ui.PairingUi
import dev.wochap.wobook.ui.components.AppBar
import dev.wochap.wobook.ui.components.ButtonTone
import dev.wochap.wobook.ui.components.ContentColumn
import dev.wochap.wobook.ui.components.LabeledField
import dev.wochap.wobook.ui.components.ScanFrame
import dev.wochap.wobook.ui.components.SyncLine
import dev.wochap.wobook.ui.components.WbButton
import dev.wochap.wobook.ui.theme.WobookType
import kotlinx.coroutines.launch
import zxingcpp.BarcodeReader
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

@Composable
fun ScanScreen(repo: AppRepository, pairing: PairingUi, onClose: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var granted by remember {
        mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED)
    }
    var asked by remember { mutableStateOf(false) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted = it; asked = true }
    LaunchedEffect(Unit) {
        pairing.connecting = null
        if (!granted) ask.launch(Manifest.permission.CAMERA)
    }
    var pasting by remember { mutableStateOf(pairing.prefill != null) }
    var pasted by remember { mutableStateOf(pairing.prefill.orEmpty()) }
    LaunchedEffect(Unit) { pairing.prefill = null }
    var error by remember { mutableStateOf<String?>(null) }
    var torch by remember { mutableStateOf(false) }
    var camera by remember { mutableStateOf<Camera?>(null) }

    /** Local validation first; no network call for bad input. */
    fun submit(text: String): Boolean {
        val parsed = QrPayload.parse(text)
        if (parsed.isFailure) {
            error = parsed.exceptionOrNull()?.message
            return false
        }
        error = null
        pairing.lastPayload = text.trim()
        pairing.peerName = parsed.getOrNull()?.name
        scope.launch {
            runCatching { repo.joinPairing(text.trim()) }.onFailure { error = it.userMessage() }
        }
        return true
    }

    LaunchedEffect(Unit) {
        val payload = pairing.lastPayload
        if (pairing.retry && payload != null) {
            pairing.retry = false
            submit(payload)
        }
    }

    val connecting = pairing.connecting
    val frame = when {
        connecting != null -> "pair-connecting"
        !granted && asked -> "pair-camera"
        else -> "pair-scan"
    }

    ContentColumn(Modifier.statusBarsPadding().navigationBarsPadding().imePadding().testTag(frame)) {
        AppBar("Scan a device's code", PhosphorIcons.Regular.X, onClose, navigationLabel = "Close")
        SyncLine(connecting != null)
        if (connecting != null) {
            Column(Modifier.fillMaxWidth().padding(24.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text("Connecting to ${connecting.peer}…", style = MaterialTheme.typography.titleMedium)
                Text("via ${connecting.via} · ${connecting.address.substringBeforeLast(':')}", style = WobookType.mono, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            return@ContentColumn
        }
        Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
            when {
                pasting -> PasteBox(pasted, { pasted = it; error = null }, error) { submit(pasted) }
                granted -> {
                    CameraPreview(onCamera = { camera = it }) { submit(it) }
                    ScanFrame()
                    IconButton(
                        onClick = { torch = !torch; camera?.cameraControl?.enableTorch(torch) },
                        modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 24.dp).size(56.dp)
                            .background(MaterialTheme.colorScheme.surfaceContainer, MaterialTheme.shapes.medium).testTag("scan-torch"),
                    ) {
                        Icon(PhosphorIcons.Regular.Flashlight, if (torch) "Torch off" else "Torch on", Modifier.size(24.dp),
                            tint = if (torch) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface)
                    }
                }
                asked -> CameraDenied {
                    context.startActivity(
                        Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null))
                            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                    )
                }
                else -> Unit
            }
        }
        Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            if (!pasting) {
                Text(
                    buildAnnotatedString {
                        append("On a desktop run ")
                        withStyle(SpanStyle(fontFamily = WobookType.mono.fontFamily)) { append("wobook pair") }
                        append(". On a phone open Devices → Show my QR.")
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                error?.let { Text(it, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.error, modifier = Modifier.testTag("scan-error")) }
                WbButton(
                    "Paste code instead", { pasting = true }, Modifier.fillMaxWidth().testTag("scan-paste"),
                    tone = ButtonTone.Secondary, icon = PhosphorIcons.Regular.ClipboardText,
                )
            }
        }
    }
}

@Composable
private fun PasteBox(value: String, onChange: (String) -> Unit, error: String?, onConnect: () -> Unit) {
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("Paste the pairing code", style = MaterialTheme.typography.titleMedium)
        LabeledField(
            "Pairing code", value, onChange, mono = true, singleLine = false, focusRequester = focus,
            imeAction = ImeAction.Done, onDone = onConnect, tag = "paste-input", placeholder = "{\"v\":1,…}",
        )
        error?.let { Text(it, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.error, modifier = Modifier.testTag("paste-error")) }
        WbButton("Connect", onConnect, Modifier.fillMaxWidth().testTag("paste-connect"), enabled = value.isNotBlank())
    }
}

@Composable
private fun CameraDenied(onOpenSettings: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(24.dp), verticalArrangement = Arrangement.spacedBy(12.dp), horizontalAlignment = Alignment.Start) {
        Icon(PhosphorIcons.Regular.CameraSlash, null, Modifier.size(32.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Text("Camera access is off", style = MaterialTheme.typography.titleLarge)
        Text(
            "wobook only uses the camera to read pairing codes. Allow it in system settings, or paste the code as text.",
            style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row { WbButton("Open settings", onOpenSettings, Modifier.testTag("camera-settings"), height = 40.dp) }
    }
}

/** CameraX preview + zxing-cpp analysis (adapted from session-tap). */
@Composable
private fun CameraPreview(onCamera: (Camera) -> Unit, onScanned: (String) -> Boolean) {
    val context = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val executor = remember { Executors.newSingleThreadExecutor() }
    val done = remember { AtomicBoolean(false) }
    val reader = remember {
        BarcodeReader().apply {
            options.formats = setOf(BarcodeReader.Format.QR_CODE)
            options.tryInvert = true
            options.tryHarder = true
            options.tryRotate = true
        }
    }
    DisposableEffect(Unit) { onDispose { executor.shutdown() } }
    AndroidView(
        modifier = Modifier.fillMaxSize(),
        factory = { ctx ->
            val view = PreviewView(ctx).apply { scaleType = PreviewView.ScaleType.FILL_CENTER }
            val future = ProcessCameraProvider.getInstance(ctx)
            future.addListener({
                val provider = runCatching { future.get() }.getOrNull() ?: return@addListener
                val preview = Preview.Builder().build().also { it.surfaceProvider = view.surfaceProvider }
                val analysis = ImageAnalysis.Builder()
                    .setResolutionSelector(
                        ResolutionSelector.Builder()
                            .setResolutionStrategy(ResolutionStrategy(Size(1920, 1080), ResolutionStrategy.FALLBACK_RULE_CLOSEST_LOWER_THEN_HIGHER))
                            .build(),
                    )
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                analysis.setAnalyzer(executor) { image ->
                    image.use {
                        if (done.get()) return@use
                        val text = runCatching { reader.read(it) }.getOrNull()?.firstOrNull()?.text ?: return@use
                        ContextCompat.getMainExecutor(context).execute {
                            if (!done.get() && onScanned(text)) done.set(true)
                        }
                    }
                }
                provider.unbindAll()
                runCatching {
                    onCamera(provider.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis))
                }
            }, ContextCompat.getMainExecutor(ctx))
            view
        },
    )
}
