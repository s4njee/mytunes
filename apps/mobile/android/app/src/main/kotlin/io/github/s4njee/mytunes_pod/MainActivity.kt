package io.github.s4njee.mytunes_pod

import android.content.Context
import android.os.Bundle
import io.flutter.embedding.android.FlutterActivity

class MainActivity : FlutterActivity() {
    private external fun initializeNativeAudio(context: Context)

    override fun onCreate(savedInstanceState: Bundle?) {
        System.loadLibrary("mt_ffi")
        initializeNativeAudio(applicationContext)
        super.onCreate(savedInstanceState)
    }
}
