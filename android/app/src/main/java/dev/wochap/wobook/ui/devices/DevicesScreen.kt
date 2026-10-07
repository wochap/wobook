package dev.wochap.wobook.ui.devices

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.adamglin.PhosphorIcons
import com.adamglin.phosphoricons.Regular
import com.adamglin.phosphoricons.regular.ArrowLeft
import com.adamglin.phosphoricons.regular.LinkBreak
import com.adamglin.phosphoricons.regular.PencilSimple
import com.adamglin.phosphoricons.regular.QrCode
import com.adamglin.phosphoricons.regular.Scan
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.data.userMessage
import dev.wochap.wobook.ffi.DeviceView
import dev.wochap.wobook.ffi.SyncState
import dev.wochap.wobook.ui.LocalSnackbar
import dev.wochap.wobook.ui.components.AppBar
import dev.wochap.wobook.ui.components.ButtonTone
import dev.wochap.wobook.ui.components.ContentColumn
import dev.wochap.wobook.ui.components.Divider
import dev.wochap.wobook.ui.components.LabeledField
import dev.wochap.wobook.ui.components.PeerRow
import dev.wochap.wobook.ui.components.SyncLine
import dev.wochap.wobook.ui.components.SyncStatusFooter
import dev.wochap.wobook.ui.components.ThisDeviceRow
import dev.wochap.wobook.ui.components.WbButton
import dev.wochap.wobook.ui.components.WbTextButton
import kotlinx.coroutines.launch

@Composable
fun DevicesScreen(repo: AppRepository, onBack: () -> Unit, onScan: () -> Unit, onShowQr: () -> Unit) {
    val scope = rememberCoroutineScope()
    val snackbar = LocalSnackbar.current
    val devices by repo.devices.collectAsState()
    val sync by repo.syncStatus.collectAsState()
    var me by remember { mutableStateOf("") }
    var menuFor by remember { mutableStateOf<String?>(null) }
    var renaming by remember { mutableStateOf<DeviceView?>(null) }
    var revoking by remember { mutableStateOf<DeviceView?>(null) }
    LaunchedEffect(Unit) {
        me = runCatching { repo.thisDevice().name }.getOrDefault("")
        repo.refreshDevices()
    }

    ContentColumn(Modifier.statusBarsPadding().navigationBarsPadding().testTag("devices")) {
        AppBar("Devices", PhosphorIcons.Regular.ArrowLeft, onBack)
        SyncLine(sync?.state is SyncState.Syncing)
        Column(Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState())) {
            ThisDeviceRow(me)
            Divider()
            devices.forEach { d ->
                PeerRow(d, onMenu = { menuFor = d.id }) {
                    DropdownMenu(
                        expanded = menuFor == d.id,
                        onDismissRequest = { menuFor = null },
                        containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
                        modifier = Modifier.testTag("devices-menu"),
                    ) {
                        DropdownMenuItem(
                            text = { Text("Rename") },
                            leadingIcon = { Icon(PhosphorIcons.Regular.PencilSimple, null, Modifier.size(20.dp)) },
                            onClick = { menuFor = null; renaming = d },
                            modifier = Modifier.testTag("menu-rename"),
                        )
                        DropdownMenuItem(
                            text = { Text("Revoke…", color = MaterialTheme.colorScheme.error) },
                            leadingIcon = { Icon(PhosphorIcons.Regular.LinkBreak, null, Modifier.size(20.dp), tint = MaterialTheme.colorScheme.error) },
                            onClick = { menuFor = null; revoking = d },
                            modifier = Modifier.testTag("menu-revoke"),
                        )
                    }
                }
            }
            Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                WbButton("Scan QR", onScan, Modifier.testTag("devices-scan"), icon = PhosphorIcons.Regular.Scan, height = 40.dp)
                WbButton("Show my QR", onShowQr, Modifier.testTag("devices-show"), tone = ButtonTone.Secondary, icon = PhosphorIcons.Regular.QrCode, height = 40.dp)
            }
        }
        SyncStatusFooter(sync)
    }

    renaming?.let { d ->
        var name by remember(d.id) { mutableStateOf(d.name) }
        AlertDialog(
            onDismissRequest = { renaming = null },
            title = { Text("Rename ${d.name}") },
            text = { LabeledField("Device name", name, { name = it }, imeAction = ImeAction.Done, tag = "rename-input") },
            confirmButton = {
                WbTextButton("Rename", {
                    renaming = null
                    scope.launch { runCatching { repo.renameDevice(d.id, name) }.onFailure { snackbar.showSnackbar(it.userMessage()) } }
                }, Modifier.testTag("rename-confirm"))
            },
            dismissButton = { WbTextButton("Cancel", { renaming = null }, color = MaterialTheme.colorScheme.onSurfaceVariant) },
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
            shape = MaterialTheme.shapes.large,
        )
    }

    // Revoke is the only confirmation dialog in the app.
    revoking?.let { d ->
        AlertDialog(
            onDismissRequest = { revoking = null },
            title = { Text("Revoke ${d.name}?") },
            text = {
                Text(
                    "It will stop syncing with every device and must be paired again from scratch. " +
                        "Its local copy of your bookmarks is not deleted. This can't be undone.",
                )
            },
            confirmButton = {
                WbTextButton("Revoke", {
                    revoking = null
                    scope.launch { runCatching { repo.revokeDevice(d.id) }.onFailure { snackbar.showSnackbar(it.userMessage()) } }
                }, Modifier.testTag("revoke-confirm"), color = MaterialTheme.colorScheme.error)
            },
            dismissButton = { WbTextButton("Cancel", { revoking = null }, color = MaterialTheme.colorScheme.onSurfaceVariant) },
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
            shape = MaterialTheme.shapes.large,
            modifier = Modifier.testTag("devices-revoke"),
        )
    }
}
