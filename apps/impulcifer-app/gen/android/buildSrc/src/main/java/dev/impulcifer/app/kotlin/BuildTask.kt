import java.io.File
import org.apache.tools.ant.taskdefs.condition.Os
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.logging.LogLevel
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.TaskAction

open class BuildTask : DefaultTask() {
    @Input
    var rootDirRel: String? = null
    @Input
    var target: String? = null
    @Input
    var release: Boolean? = null

    // The Tauri CLI is either cargo's subcommand (`cargo install tauri-cli`,
    // which puts `cargo-tauri` on PATH) or the npm package's own executable
    // (`npm install -g @tauri-apps/cli`, which the workflows use). Which one is
    // there is read from PATH, so a build that fails is not run a second time
    // under the other name.
    @TaskAction
    fun assemble() {
        val windows = Os.isFamily(Os.FAMILY_WINDOWS)
        val extensions = if (windows) listOf(".exe", ".cmd", ".bat", "") else listOf("")
        val onPath = { name: String ->
            (System.getenv("PATH") ?: "").split(File.pathSeparator).any { directory ->
                extensions.any { File(directory, "$name$it").isFile }
            }
        }
        val (executable, prefix) =
            if (onPath("cargo-tauri")) "cargo" to listOf("tauri") else "tauri" to emptyList()
        // Windows starts a program only under its full file name; the other
        // platforms have one name, so a failed build is reported as it is.
        val names = if (windows) {
            listOf(executable, "$executable.exe", "$executable.cmd", "$executable.bat")
        } else {
            listOf(executable)
        }
        var lastException: Exception? = null
        for (name in names) {
            try {
                runTauriCli(name, prefix)
                return
            } catch (e: Exception) {
                lastException = e
            }
        }
        throw lastException ?: GradleException("the Tauri CLI could not be started")
    }

    fun runTauriCli(executable: String, prefix: List<String>) {
        val rootDirRel = rootDirRel ?: throw GradleException("rootDirRel cannot be null")
        val target = target ?: throw GradleException("target cannot be null")
        val release = release ?: throw GradleException("release cannot be null")
        val args = prefix + listOf("android", "android-studio-script");

        project.exec {
            workingDir(File(project.projectDir, rootDirRel))
            executable(executable)
            args(args)
            if (project.logger.isEnabled(LogLevel.DEBUG)) {
                args("-vv")
            } else if (project.logger.isEnabled(LogLevel.INFO)) {
                args("-v")
            }
            if (release) {
                args("--release")
            }
            args(listOf("--target", target))
        }.assertNormalExitValue()
    }
}