package com.ark.android

import android.os.Bundle
import android.widget.LinearLayout
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import android.widget.TextView

import uniffi.rpc_core.call;

class MainActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)


        var res1 = call("factorial", listOf("10"))
        
        val textView = TextView(this).apply {
            text = res1
            textSize = 24f
            setPadding(16, 16, 16, 16)
        }

        val res2 = call("sum", listOf("[1,2,3,4,5,6]","10"))


        val textView2 = TextView(this).apply {
            text = res2
            textSize = 24f
            setPadding(16, 16, 16, 16)
        }
        
        val bothTextViews = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(textView)
            addView(textView2)
        }

        setContentView(bothTextViews)
    
    }
}