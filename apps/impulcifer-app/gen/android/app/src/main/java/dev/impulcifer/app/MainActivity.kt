package dev.impulcifer.app

import android.content.res.Configuration
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.system.Os
import android.view.View
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  /** The theme saved in the app's settings: "dark", "light" or "system". */
  private var themeSetting = "system"

  override fun onCreate(savedInstanceState: Bundle?) {
    // The Rust service makes its temporary files where TMPDIR points. An app
    // process has no TMPDIR and Rust then falls back to /data/local/tmp, which
    // an app cannot write. Set once per process, before the Rust code starts:
    // on a later activity its threads may already be reading the environment.
    val cache = cacheDir.absolutePath
    if (Os.getenv("TMPDIR") != cache) {
      Os.setenv("TMPDIR", cache, true)
    }
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // Android draws the app behind the status and navigation bars. The page
    // does not know their sizes, so the web view is kept between them (and
    // above the keyboard) here; the strips left around it show the window
    // background, which applyPageTheme keeps equal to the page's own.
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or
          WindowInsetsCompat.Type.displayCutout() or
          WindowInsetsCompat.Type.ime()
      )
      view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      WindowInsetsCompat.CONSUMED
    }
    applyPageTheme(themeSetting)
  }

  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    // The system switched between light and dark while the app was open.
    applyPageTheme(themeSetting)
  }

  /** Whether the system is in its dark theme right now. */
  fun systemIsDark(): Boolean =
    (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
      Configuration.UI_MODE_NIGHT_YES

  /**
   * Paints the window behind the system bars with the page's background
   * (bg-0 of ui/styles.css) and picks bar icons that show on it. "system"
   * follows the system's theme, as the page does.
   */
  fun applyPageTheme(setting: String) {
    themeSetting = setting
    val dark = setting == "dark" || (setting != "light" && systemIsDark())
    val style = if (dark) {
      SystemBarStyle.dark(Color.TRANSPARENT)
    } else {
      SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT)
    }
    enableEdgeToEdge(style, style)
    window.setBackgroundDrawable(ColorDrawable(if (dark) PAGE_DARK else PAGE_LIGHT))
  }

  private companion object {
    val PAGE_DARK = Color.rgb(0x10, 0x12, 0x14)
    val PAGE_LIGHT = Color.rgb(0xf3, 0xf5, 0xf7)
  }
}
