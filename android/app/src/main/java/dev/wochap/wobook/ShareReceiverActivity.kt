package dev.wochap.wobook

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import dev.wochap.wobook.ui.share.ShareSheet
import dev.wochap.wobook.ui.theme.WobookTheme

/** `ACTION_SEND text/plain` target rendered as a bottom sheet over the sender. */
class ShareReceiverActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val text = intent.getStringExtra(Intent.EXTRA_TEXT).orEmpty()
        val subject = intent.getStringExtra(Intent.EXTRA_SUBJECT).orEmpty()
        setContent {
            WobookTheme {
                ShareSheet(
                    repo = wobook.repository,
                    settings = wobook.settings,
                    sharedText = text,
                    subject = subject,
                    onMore = { args ->
                        startActivity(
                            args.toIntent(Intent(this, MainActivity::class.java))
                                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP),
                        )
                        finish()
                    },
                    onFinish = { finish() },
                )
            }
        }
    }

    override fun finish() {
        super.finish()
        @Suppress("DEPRECATION")
        overridePendingTransition(0, 0)
    }
}

