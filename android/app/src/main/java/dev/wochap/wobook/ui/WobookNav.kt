package dev.wochap.wobook.ui

import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp
import androidx.navigation.NavController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.data.Settings
import dev.wochap.wobook.data.SettingsState
import dev.wochap.wobook.ffi.PairingEvent
import dev.wochap.wobook.ui.detail.DetailScreen
import dev.wochap.wobook.ui.devices.DevicesScreen
import dev.wochap.wobook.ui.form.FormArgs
import dev.wochap.wobook.ui.form.FormScreen
import dev.wochap.wobook.ui.home.HomeScreen
import dev.wochap.wobook.ui.onboarding.OnboardingChooseScreen
import dev.wochap.wobook.ui.onboarding.OnboardingNameScreen
import dev.wochap.wobook.ui.pairing.ConfirmScreen
import dev.wochap.wobook.ui.pairing.PairingResultScreen
import dev.wochap.wobook.ui.pairing.ScanScreen
import dev.wochap.wobook.ui.pairing.ShowQrScreen
import dev.wochap.wobook.ui.settings.SettingsScreen
import dev.wochap.wobook.ui.theme.Latte
import dev.wochap.wobook.ui.theme.Mocha
import dev.wochap.wobook.ui.theme.Wb

object Routes {
    const val ONB_NAME = "onboarding/name"
    const val ONB_CHOOSE = "onboarding/choose"
    const val HOME = "home"
    const val DETAIL = "detail?url={url}"
    const val FORM = "form?url={url}&title={title}&description={description}&tags={tags}"
    const val SCAN = "pairing/scan"
    const val SHOW = "pairing/show"
    const val CONFIRM = "pairing/confirm/{id}"
    const val RESULT = "pairing/result"
    const val DEVICES = "devices"
    const val SETTINGS = "settings"

    fun detail(url: String) = "detail?url=${Uri.encode(url)}"
    fun form(args: FormArgs = FormArgs()) = buildString {
        append("form?url=").append(Uri.encode(args.url))
        append("&title=").append(Uri.encode(args.title))
        append("&description=").append(Uri.encode(args.description))
        append("&tags=").append(Uri.encode(args.tags.joinToString(",")))
    }
    fun confirm(id: String) = "pairing/confirm/$id"
}

/** Home search state kept above the NavHost so it survives Detail and back. */
class HomeState {
    var query by mutableStateOf("")
    val selected = mutableStateListOf<String>()
}

/** Pairing progress shared by Scan, Show, Confirm and Result. */
class PairingUi {
    var connecting by mutableStateOf<PairingEvent.Connecting?>(null)
    var result by mutableStateOf<PairingEvent?>(null)
    var confirmations = mutableStateListOf<dev.wochap.wobook.ffi.PairingConfirmation>()
    /** Last payload joined and its device name, for Retry and result copy. */
    var lastPayload by mutableStateOf<String?>(null)
    var peerName by mutableStateOf<String?>(null)
    /** Scan should re-join [lastPayload] on entry. */
    var retry by mutableStateOf(false)
    /** Code from a `wobook://pair` link: Scan opens in paste mode with it. */
    var prefill by mutableStateOf<String?>(null)
}

@Composable
fun WobookRoot(
    repo: AppRepository,
    settings: Settings,
    pendingForm: FormArgs?,
    onFormConsumed: () -> Unit,
    pendingPair: String?,
    onPairConsumed: () -> Unit,
) {
    val settingsState by settings.state.collectAsState(initial = null)
    val openError by repo.openError.collectAsState()
    val snackbar = remember { SnackbarHostState() }
    LaunchedEffect(Unit) { runCatching { repo.app() } }

    CompositionLocalProvider(LocalSnackbar provides snackbar) {
        // Expose testTags as resource ids so Maestro selects by design frame id.
        Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).semantics { testTagsAsResourceId = true }) {
            val state = settingsState
            when {
                openError != null -> OpenError(openError!!)
                state == null -> Unit
                else -> WobookNav(repo, settings, state, pendingForm, onFormConsumed, pendingPair, onPairConsumed)
            }
            SnackbarHost(
                snackbar,
                modifier = Modifier.align(Alignment.BottomCenter).navigationBarsPadding().imePadding().padding(bottom = 72.dp),
            ) { data ->
                Snackbar(
                    data,
                    containerColor = MaterialTheme.colorScheme.inverseSurface,
                    contentColor = MaterialTheme.colorScheme.inverseOnSurface,
                    // Snackbar sits on the inverse surface: use the other flavour's blue.
                    actionColor = if (Wb.colors.isDark) Latte.blue else Mocha.blue,
                    shape = MaterialTheme.shapes.small,
                )
            }
        }
    }
}

@Composable
private fun OpenError(message: String) {
    Column(Modifier.fillMaxSize().systemBarsPadding().padding(24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("wobook couldn't open its library", style = MaterialTheme.typography.titleLarge)
        Text(message, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun WobookNav(
    repo: AppRepository,
    settings: Settings,
    state: SettingsState,
    pendingForm: FormArgs?,
    onFormConsumed: () -> Unit,
    pendingPair: String?,
    onPairConsumed: () -> Unit,
) {
    val nav = rememberNavController()
    val home = remember { HomeState() }
    val pairing = remember { PairingUi() }
    val snackbar = LocalSnackbar.current
    // Fixed for this NavHost: a changing start destination would reset the back stack.
    val start = remember { if (state.onboardingDone) Routes.HOME else Routes.ONB_NAME }

    // Pairing events drive navigation from any screen.
    LaunchedEffect(Unit) {
        repo.pairingEvents.collect { event ->
            when (event) {
                is PairingEvent.Connecting -> pairing.connecting = event
                is PairingEvent.ConfirmRequired -> {
                    pairing.connecting = null
                    pairing.confirmations.removeAll { it.id == event.confirmation.id }
                    pairing.confirmations.add(event.confirmation)
                    nav.navigate(Routes.confirm(event.confirmation.id)) { launchSingleTop = true }
                }
                is PairingEvent.Completed -> {
                    pairing.connecting = null
                    pairing.result = null
                    settings.setOnboardingDone(true)
                    nav.toDevicesOverHome()
                    snackbar.showSnackbar("Paired with ${event.device.name}")
                }
                is PairingEvent.Expired, is PairingEvent.Rejected, is PairingEvent.Unreachable, is PairingEvent.Failed -> {
                    pairing.connecting = null
                    pairing.result = event
                    nav.navigate(Routes.RESULT) { launchSingleTop = true }
                }
            }
        }
    }

    LaunchedEffect(pendingForm) {
        if (pendingForm != null && state.onboardingDone) {
            nav.navigate(Routes.form(pendingForm))
            onFormConsumed()
        }
    }

    LaunchedEffect(pendingPair) {
        if (pendingPair != null) {
            pairing.prefill = pendingPair
            nav.navigate(Routes.SCAN) { launchSingleTop = true }
            onPairConsumed()
        }
    }

    NavHost(nav, startDestination = start, modifier = Modifier.fillMaxSize()) {
        composable(Routes.ONB_NAME) {
            OnboardingNameScreen(repo) { nav.navigate(Routes.ONB_CHOOSE) }
        }
        composable(Routes.ONB_CHOOSE) {
            OnboardingChooseScreen(
                repo = repo,
                onPair = { nav.navigate(Routes.SCAN) },
                onFresh = {
                    nav.navigate(Routes.HOME) { popUpTo(0) { inclusive = true } }
                },
                settings = settings,
            )
        }
        composable(Routes.HOME) {
            HomeScreen(
                repo = repo,
                state = home,
                tap = state.tap,
                loadIcons = state.loadIcons,
                onDetail = { nav.navigate(Routes.detail(it)) },
                onAdd = { url -> nav.navigate(Routes.form(FormArgs(url = url))) },
                onEdit = { nav.navigate(Routes.form(FormArgs(url = it, edit = true))) },
                onSettings = { nav.navigate(Routes.SETTINGS) },
                onScan = { nav.navigate(Routes.SCAN) },
                onShowQr = { nav.navigate(Routes.SHOW) },
            )
        }
        composable(Routes.DETAIL, arguments = listOf(navArgument("url") { type = NavType.StringType; defaultValue = "" })) { entry ->
            val url = entry.arguments?.getString("url").orEmpty()
            DetailScreen(
                repo = repo,
                url = url,
                onBack = { nav.popBackStack() },
                onEdit = { nav.navigate(Routes.form(FormArgs(url = url, edit = true))) },
                onDeleted = { nav.popBackStack(Routes.HOME, inclusive = false) },
            )
        }
        composable(
            Routes.FORM,
            arguments = listOf("url", "title", "description", "tags").map { name ->
                navArgument(name) { type = NavType.StringType; defaultValue = "" }
            },
        ) { entry ->
            val a = entry.arguments
            val args = FormArgs(
                url = a?.getString("url").orEmpty(),
                title = a?.getString("title").orEmpty(),
                description = a?.getString("description").orEmpty(),
                tags = a?.getString("tags").orEmpty().split(',').filter { it.isNotBlank() },
            )
            FormScreen(
                repo = repo,
                args = args,
                autoFetch = state.autoFetch,
                onClose = { nav.popBackStack() },
                onSaved = { url ->
                    nav.navigate(Routes.detail(url)) {
                        popUpTo(Routes.HOME) { inclusive = false }
                    }
                },
                onDeleted = { nav.popBackStack(Routes.HOME, inclusive = false) },
            )
        }
        composable(Routes.SCAN) {
            ScanScreen(repo = repo, pairing = pairing, onClose = { nav.popBackStack() })
        }
        composable(Routes.SHOW) {
            ShowQrScreen(repo = repo, onBack = { nav.popBackStack() })
        }
        composable(Routes.CONFIRM, arguments = listOf(navArgument("id") { type = NavType.StringType })) { entry ->
            val id = entry.arguments?.getString("id").orEmpty()
            ConfirmScreen(repo = repo, pairing = pairing, id = id, onRejected = { nav.toDevicesOverHome() })
        }
        composable(Routes.RESULT) {
            PairingResultScreen(
                pairing = pairing,
                onClose = { nav.toDevicesOverHome() },
                onScanAgain = { nav.navigate(Routes.SCAN) { popUpTo(Routes.RESULT) { inclusive = true } } },
            )
        }
        composable(Routes.DEVICES) {
            DevicesScreen(
                repo = repo,
                onBack = { if (!nav.popBackStack()) nav.navigate(Routes.HOME) },
                onScan = { nav.navigate(Routes.SCAN) },
                onShowQr = { nav.navigate(Routes.SHOW) },
            )
        }
        composable(Routes.SETTINGS) {
            SettingsScreen(
                repo = repo,
                settings = settings,
                state = state,
                onBack = { nav.popBackStack() },
                onDevices = { nav.navigate(Routes.DEVICES) },
            )
        }
    }
}

/**
 * Show Devices with Home directly beneath it. During onboarding Home is not on
 * the back stack, so the whole stack is replaced to keep onboarding and
 * pairing screens out of reach of Back.
 */
private fun NavController.toDevicesOverHome() {
    if (runCatching { getBackStackEntry(Routes.HOME) }.isSuccess) {
        navigate(Routes.DEVICES) {
            popUpTo(Routes.HOME) { inclusive = false }
            launchSingleTop = true
        }
    } else {
        navigate(Routes.HOME) { popUpTo(0) { inclusive = true } }
        navigate(Routes.DEVICES) { launchSingleTop = true }
    }
}
