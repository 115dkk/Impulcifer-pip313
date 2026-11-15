package dev.impulcifer.app

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.storage.StorageManager
import android.os.storage.StorageVolume
import android.provider.DocumentsContract
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class OpenFolderArgs {
  var path: String = ""
}

@InvokeArg
class ApplyThemeArgs {
  var theme: String = "system"
}

/**
 * What the Rust shell asks the platform (ADR 0020, src/android.rs). The page
 * never calls these: every command is reached from Rust, which decides what the
 * page may know and do.
 */
@TauriPlugin
class ImpulciferPlugin(private val activity: Activity) : Plugin(activity) {

  /** The Android version and the device, for the system information. */
  @Command
  fun device(invoke: Invoke) {
    val result = JSObject()
    result.put("release", Build.VERSION.RELEASE ?: "")
    result.put("sdk", Build.VERSION.SDK_INT)
    result.put("manufacturer", Build.MANUFACTURER ?: "")
    result.put("model", Build.MODEL ?: "")
    invoke.resolve(result)
  }

  /**
   * Whether the app holds "all files access". The service reads measurement
   * folders and writes its results by path, anywhere on shared storage.
   */
  @Command
  fun storageAccess(invoke: Invoke) {
    val result = JSObject()
    result.put("granted", Environment.isExternalStorageManager())
    invoke.resolve(result)
  }

  /**
   * Opens the system screen with this app's "all files access" switch, or the
   * list of apps when a device has no per-app screen.
   */
  @Command
  fun requestStorageAccess(invoke: Invoke) {
    val forThisApp = Intent(
      Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
      Uri.parse("package:${activity.packageName}")
    )
    val forAllApps = Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION)
    val result = JSObject()
    result.put("opened", start(forThisApp) || start(forAllApps))
    invoke.resolve(result)
  }

  /** The mounted shared storage volumes, the device's own first. */
  @Command
  fun storageVolumes(invoke: Invoke) {
    val volumes = JSArray()
    for (volume in mountedVolumes()) {
      val directory = volume.directory ?: continue
      val entry = JSObject()
      entry.put("path", directory.absolutePath)
      entry.put("label", volume.getDescription(activity) ?: directory.name)
      entry.put("removable", volume.isRemovable)
      volumes.put(entry)
    }
    val result = JSObject()
    result.put("volumes", volumes)
    invoke.resolve(result)
  }

  /**
   * Shows a folder of shared storage in the system file manager. Android has
   * no agreed way to do this; the documents provider's folder address is the
   * one most file managers answer. `opened` is false when none does.
   */
  @Command
  fun openFolder(invoke: Invoke) {
    val args = invoke.parseArgs(OpenFolderArgs::class.java)
    val result = JSObject()
    val document = documentId(File(args.path))
    if (document == null) {
      result.put("opened", false)
    } else {
      val uri = DocumentsContract.buildDocumentUri(DOCUMENTS_AUTHORITY, document)
      val intent = Intent(Intent.ACTION_VIEW)
      intent.setDataAndType(uri, DocumentsContract.Document.MIME_TYPE_DIR)
      intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      result.put("opened", start(intent))
    }
    invoke.resolve(result)
  }

  /** Whether the system is in its dark theme. */
  @Command
  fun systemAppearance(invoke: Invoke) {
    val result = JSObject()
    result.put("dark", (activity as? MainActivity)?.systemIsDark() ?: false)
    invoke.resolve(result)
  }

  /** Matches the window and the system bars to the page's theme setting. */
  @Command
  fun applyTheme(invoke: Invoke) {
    val args = invoke.parseArgs(ApplyThemeArgs::class.java)
    val main = activity as? MainActivity
    if (main != null) {
      main.runOnUiThread { main.applyPageTheme(args.theme) }
    }
    invoke.resolve(JSObject())
  }

  private fun mountedVolumes(): List<StorageVolume> {
    val manager = activity.getSystemService(StorageManager::class.java) ?: return emptyList()
    return manager.storageVolumes
      .filter { it.state == Environment.MEDIA_MOUNTED }
      .sortedByDescending { it.isPrimary }
  }

  /**
   * The documents provider's id of a folder: `primary:Download/demo` on the
   * device's own storage, `<volume id>:<path>` on a card. Null for a folder
   * outside every mounted volume.
   */
  private fun documentId(folder: File): String? {
    val path = folder.absolutePath
    for (volume in mountedVolumes()) {
      val root = volume.directory?.absolutePath ?: continue
      val relative = when {
        path == root -> ""
        path.startsWith("$root/") -> path.substring(root.length + 1)
        else -> continue
      }
      val name = if (volume.isPrimary) "primary" else volume.uuid ?: continue
      return "$name:$relative"
    }
    return null
  }

  private fun start(intent: Intent): Boolean =
    try {
      intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      activity.startActivity(intent)
      true
    } catch (_: Exception) {
      // No activity answers the intent, or the system refused it.
      false
    }

  private companion object {
    const val DOCUMENTS_AUTHORITY = "com.android.externalstorage.documents"
  }
}
