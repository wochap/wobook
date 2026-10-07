package dev.wochap.wobook

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import dev.wochap.wobook.ui.WobookRoot
import dev.wochap.wobook.ui.form.FormArgs
import dev.wochap.wobook.ui.theme.WobookTheme

class MainActivity : ComponentActivity() {
    private var pendingForm by mutableStateOf<FormArgs?>(null)
    private var pendingPair by mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        pendingForm = FormArgs.fromIntent(intent)
        pendingPair = pairCode(intent)
        setContent {
            WobookTheme {
                WobookRoot(wobook.repository, wobook.settings, pendingForm, { pendingForm = null }, pendingPair) { pendingPair = null }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        FormArgs.fromIntent(intent)?.let { pendingForm = it }
        pairCode(intent)?.let { pendingPair = it }
    }

    private fun pairCode(intent: Intent?): String? =
        intent?.data?.takeIf { it.scheme == "wobook" && it.host == "pair" }?.getQueryParameter("code")
}
