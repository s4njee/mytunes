package io.github.s4njee.mytunes_pod

import android.app.Application

class PodApplication : Application() {
    private external fun initializeNativeAudio()

    override fun onCreate() {
        super.onCreate()
        System.loadLibrary("mt_ffi")
        initializeNativeAudio()
    }
}
