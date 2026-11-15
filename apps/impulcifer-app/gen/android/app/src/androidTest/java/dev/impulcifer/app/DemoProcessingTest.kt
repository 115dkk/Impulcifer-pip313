package dev.impulcifer.app

import android.graphics.Bitmap
import android.os.Environment
import android.os.SystemClock
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.io.FileOutputStream
import java.security.MessageDigest
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The installed app on a device (ADR 0020, item 12): the real page in the real
 * web view talks to the real service, which processes the demo measurement
 * folder into BRIR files, with room correction and headphone compensation
 * on, and then restores an output that was deleted.
 *
 * The page is driven through its own elements (a value typed into the folder
 * field, a click on the button), by script, because a test cannot touch inside
 * a web view any other way. What it proves is everything behind those
 * elements: the shell, the Kotlin plugin, storage access, the bundled test
 * signal, the temporary folder, the pipeline and the files it writes.
 *
 * android.yml pushes `data/demo` to [demo], grants all-files access with
 * `appops`, and pulls [evidence] (screenshots and result.json) afterwards.
 */
@RunWith(AndroidJUnit4::class)
class DemoProcessingTest {
  private val arguments = InstrumentationRegistry.getArguments()
  private val demo = File(arguments.getString("demoFolder") ?: "/sdcard/Download/impulcifer-demo")
  private val evidence = File(arguments.getString("evidenceFolder") ?: "/sdcard/Download/impulcifer-test")
  private val result = JSONObject()

  private lateinit var scenario: ActivityScenario<MainActivity>
  private lateinit var webView: WebView

  @Before
  fun launch() {
    evidence.mkdirs()
    scenario = ActivityScenario.launch(MainActivity::class.java)
    webView = waitFor("the web view", 120_000) { findWebView() }
  }

  // The activity is left open: MainActivity ends the process when it is
  // destroyed, and the process is this test's own. android_emulator_test.sh
  // tells the runner not to finish activities either.
  @After
  fun record() {
    File(evidence, "result.json").writeText(result.toString(2))
  }

  @Test
  fun processesTheDemoFolderAndRestoresAnOutput() {
    assertTrue("android.yml pushes the demo measurement to $demo", File(demo, "FL,FR.wav").isFile)
    for (name in OUTPUTS) File(demo, name).delete()

    // The page starts: bootstrap answered and every part is wired.
    waitUntil("the page to start", 180_000, "document.documentElement.dataset.booted === 'true'")
    assertEquals("android", evaluate("document.documentElement.dataset.page"))
    shot("01-processing")

    // What the service says about this device.
    val bootstrap = call("bootstrap").getJSONObject("data")
    assertEquals("android", bootstrap.getString("platform"))
    assertEquals("apk", bootstrap.getString("install_kind"))
    assertFalse(bootstrap.getJSONObject("capabilities").getBoolean("recording"))
    val access = call("get_storage_access").getJSONObject("data")
    assertEquals("android.yml grants all-files access before the test", "granted", access.getString("state"))
    val system = call("get_system_info").getJSONObject("data")
    result.put("system", system)
    assertTrue(system.getString("os"), system.getString("os").startsWith("Android "))
    assertTrue(system.optString("device"), system.optString("device").isNotBlank())
    val webview = system.getJSONObject("runtime").optString("webview")
    assertTrue(webview, webview.startsWith("Android System WebView "))

    // The folder list the chooser shows comes from the real file system.
    val listing = call("list_directory", JSONObject().put("path", demo.parent).put("kind", "dir"))
      .getJSONObject("data")
    val primary = Environment.getExternalStorageDirectory().absolutePath
    assertEquals(primary, listing.getJSONArray("roots").getJSONObject(0).getString("path"))
    assertTrue(
      "the demo folder is listed in ${demo.parent}",
      names(listing.getJSONArray("entries")).contains(demo.name)
    )

    // The chooser opens from the browse button and lists the volume.
    click("[data-browse][data-target='bf-dir-path']")
    waitUntil("the chooser to open", 30_000, visible("chooser"))
    waitUntil("the chooser to list a folder", 30_000, "document.getElementById('chooser-list').children.length > 0")
    shot("02-chooser")
    click("#chooser-close")
    waitUntil("the chooser to close", 30_000, "!(${visible("chooser")})")

    // Processing with the two corrections the demo folder has recordings for:
    // the folder, room correction on, headphone compensation on, the button.
    type("bf-dir-path", demo.absolutePath)
    for (section in listOf("dis-room", "dis-headphone")) {
      click("#$section [data-disclosure]")
      waitUntil("$section to open", 30_000, "document.getElementById('$section').classList.contains('open')")
    }
    val request = JSONObject(evaluate("JSON.stringify(gatherBrirPayload())") as String)
    result.put("brir_request", request)
    assertTrue(request.getBoolean("do_room_correction"))
    assertTrue(request.getBoolean("do_headphone_compensation"))
    waitUntil("the Generate button to be ready", 60_000, "!document.getElementById('btn-generate-brir').disabled")
    click("#btn-generate-brir")
    val started = SystemClock.elapsedRealtime()
    val brir = waitForJob("brir", 30 * 60_000L, "03-processing-running")
    result.put("brir_seconds", (SystemClock.elapsedRealtime() - started) / 1000.0)
    shot("04-processing-done")
    assertEquals("the BRIR job: ${log()}", "succeeded", brir)
    val written = JSONObject()
    for (name in OUTPUTS) {
      val file = File(demo, name)
      assertTrue("$name is written in the measurement folder", file.isFile && file.length() > 0)
      written.put(name, JSONObject().put("bytes", file.length()).put("sha256", sha256(file)))
    }
    result.put("outputs", written)

    // Output recovery: delete one output and bring it back from the others.
    val missing = File(demo, RESTORED)
    val original = written.getJSONObject(RESTORED)
    assertTrue(missing.delete())
    click("#nav-recovery")
    type("recovery-dir-path", demo.absolutePath)
    waitUntil(
      "the recovery plan",
      120_000,
      "document.getElementById('recovery-inventory').dataset.state === 'ready'"
    )
    shot("05-recovery-ready")
    click("#btn-start-recovery")
    val recovery = waitForJob("output_recovery", 10 * 60_000L, null)
    shot("06-recovery-done")
    assertEquals("the recovery job: ${log()}", "succeeded", recovery)
    assertTrue("$RESTORED is restored", missing.isFile && missing.length() > 0)
    result.put(
      "restored",
      JSONObject()
        .put("file", RESTORED)
        .put("bytes", missing.length())
        .put("same_bytes_as_first_run", sha256(missing) == original.getString("sha256"))
    )

    // The other two views, for the record.
    click("#nav-settings")
    waitUntil("the settings view", 30_000, "document.getElementById('view-settings').classList.contains('active')")
    shot("07-settings")
    click("#nav-info")
    waitUntil("the information view", 30_000, "document.getElementById('view-info').classList.contains('active')")
    shot("08-info")
  }

  /** Waits for the job the page follows to end and returns its last status. */
  private fun waitForJob(kind: String, timeout: Long, runningShot: String?): String {
    val root = "document.documentElement.dataset"
    waitUntil("the $kind job to start", 60_000, "$root.jobKind === '$kind' || ${visible("ask-modal")}")
    // A question before the job (a warning to confirm) is answered with OK.
    if (evaluate(visible("ask-modal")) == true) {
      result.put("${kind}_question", evaluate("document.getElementById('ask-modal-message').textContent"))
      click("#ask-modal-ok")
      waitUntil("the $kind job to start", 60_000, "$root.jobKind === '$kind'")
    }
    if (runningShot != null) {
      SystemClock.sleep(3_000)
      shot(runningShot)
    }
    waitUntil(
      "the $kind job to end",
      timeout,
      "$root.jobKind === '$kind' && ['succeeded','failed','cancelled'].includes($root.jobStatus)"
    )
    return evaluate("$root.jobStatus") as String
  }

  private fun findWebView(): WebView? {
    val found = AtomicReference<WebView?>()
    scenario.onActivity { activity ->
      found.set(search(activity.findViewById(android.R.id.content)))
    }
    return found.get()
  }

  private fun search(view: View?): WebView? {
    if (view is WebView) return view
    if (view is ViewGroup) {
      for (index in 0 until view.childCount) {
        search(view.getChildAt(index))?.let { return it }
      }
    }
    return null
  }

  /**
   * The value of a page expression: a String, Number, Boolean, JSONObject or
   * JSONArray, or null for null and undefined. A script that throws fails the
   * test with its message.
   */
  private fun evaluate(expression: String): Any? {
    val latch = CountDownLatch(1)
    val answer = AtomicReference<String?>()
    val script = "(function(){try{var v=($expression);return JSON.stringify({value:v===undefined?null:v});}" +
      "catch(error){return JSON.stringify({error:String(error)});}})()"
    InstrumentationRegistry.getInstrumentation().runOnMainSync {
      webView.evaluateJavascript(script) { text ->
        answer.set(text)
        latch.countDown()
      }
    }
    assertTrue("the page did not answer: $expression", latch.await(60, TimeUnit.SECONDS))
    // evaluateJavascript hands the script's string back as a JSON string.
    val text = JSONTokener(answer.get() ?: "null").nextValue() as? String
    assertNotNull("the page returned nothing for: $expression", text)
    val envelope = JSONObject(text!!)
    if (envelope.has("error")) {
      throw AssertionError("the page script failed: ${envelope.getString("error")} in $expression")
    }
    val value = envelope.opt("value")
    return if (value == JSONObject.NULL) null else value
  }

  /** One IPC call through the page's bridge, as the page itself makes it. */
  private fun call(method: String, argument: JSONObject? = null): JSONObject {
    val args = if (argument == null) "" else argument.toString()
    evaluate(
      "(window.__impulciferTest = null, window.pywebview.api.$method($args)" +
        ".then(function(r){window.__impulciferTest = JSON.stringify(r);})" +
        ".catch(function(e){window.__impulciferTest = JSON.stringify({ok:false,thrown:String(e)});}), true)"
    )
    waitUntil("$method to answer", 60_000, "window.__impulciferTest !== null")
    val response = JSONObject(evaluate("window.__impulciferTest") as String)
    assertTrue("$method: $response", response.optBoolean("ok"))
    return response
  }

  private fun click(selector: String) {
    val quoted = JSONObject.quote(selector)
    assertEquals(
      "an element to click: $selector",
      true,
      evaluate("(function(){var e=document.querySelector($quoted);if(!e)return false;e.click();return true;})()")
    )
  }

  /** Puts text in a field the way typing does: the value, then `input`. */
  private fun type(id: String, text: String) {
    val quoted = JSONObject.quote(text)
    assertEquals(
      "a field to type into: $id",
      true,
      evaluate(
        "(function(){var e=document.getElementById('$id');if(!e)return false;e.value=$quoted;" +
          "e.dispatchEvent(new Event('input',{bubbles:true}));return true;})()"
      )
    )
  }

  /** A page expression that is true while the element is shown. */
  private fun visible(id: String): String =
    "(function(){var e=document.getElementById('$id');if(!e||e.hidden)return false;" +
      "var r=e.getBoundingClientRect();return r.width>0&&r.height>0;})()"

  /** The end of the page's job log, for a failure message. */
  private fun log(): String =
    (evaluate("(function(){var e=document.querySelector('[data-log]');return e?e.textContent:'';})()") as? String)
      .orEmpty()
      .takeLast(4000)

  /**
   * Waits for a page expression to be true. A page that cannot answer yet
   * (no document, a script that is not loaded) counts as "not yet"; what it
   * last said goes into the timeout message.
   */
  private fun waitUntil(what: String, timeout: Long, expression: String) {
    var last = "the expression stayed false"
    try {
      waitFor(what, timeout) {
        try {
          if (evaluate(expression) == true) true else null
        } catch (error: AssertionError) {
          last = error.message ?: error.toString()
          null
        }
      }
    } catch (error: AssertionError) {
      throw AssertionError("${error.message} ($last)")
    }
  }

  private fun <T : Any> waitFor(what: String, timeout: Long, probe: () -> T?): T {
    val deadline = SystemClock.elapsedRealtime() + timeout
    while (true) {
      probe()?.let { return it }
      if (SystemClock.elapsedRealtime() > deadline) {
        if (this::webView.isInitialized) shot("timeout")
        throw AssertionError("timed out after ${timeout / 1000} s waiting for $what")
      }
      SystemClock.sleep(500)
    }
  }

  /** The screen as the device shows it, system bars included. */
  private fun shot(name: String) {
    val bitmap = InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot() ?: return
    FileOutputStream(File(evidence, "$name.png")).use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
  }

  private fun names(entries: JSONArray): List<String> =
    (0 until entries.length()).map { entries.getJSONObject(it).getString("name") }

  private fun sha256(file: File): String {
    val digest = MessageDigest.getInstance("SHA-256")
    file.inputStream().use { input ->
      val buffer = ByteArray(1 shl 16)
      while (true) {
        val count = input.read(buffer)
        if (count < 0) break
        digest.update(buffer, 0, count)
      }
    }
    return digest.digest().joinToString("") { "%02x".format(it) }
  }

  private companion object {
    /** What a default run writes beside the recordings. */
    val OUTPUTS = listOf("hrir.wav", "hesuvi.wav", "responses.wav")
    /** The output deleted and restored: recovery rebuilds it from hrir.wav. */
    const val RESTORED = "hesuvi.wav"
  }
}
