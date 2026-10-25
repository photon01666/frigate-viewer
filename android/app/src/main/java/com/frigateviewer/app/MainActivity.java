package com.frigateviewer.app;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.pm.ApplicationInfo;
import android.graphics.Bitmap;
import android.net.http.SslError;
import android.os.Bundle;
import android.util.Log;
import android.view.WindowManager;
import android.webkit.ConsoleMessage;
import android.webkit.CookieManager;
import android.webkit.SslErrorHandler;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/**
 * The app is a thin native host around the Frigate web UI. Every bit of
 * configuration, networking and the setup server lives in the Rust library; this
 * class only owns the WebView and the platform callbacks.
 */
public class MainActivity extends Activity {

    static {
        System.loadLibrary("frigate_viewer");
    }

    private static final String TAG = "FrigateViewer";
    private static final Set<String> ACCEPTED_CERT_HOSTS = ConcurrentHashMap.newKeySet();

    private WebView webView;

    private native String nativeStart(String dataDir);
    private native void nativeStop();
    private native boolean nativeAllowInvalidCertificates();
    private native boolean nativeKeepScreenOn();
    private native String nativeGetOverlayScript();

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        webView = new WebView(this);
        webView.setBackgroundColor(0xFF0A0E13);
        configureWebView(webView);
        setContentView(webView);

        String initialUrl = nativeStart(getFilesDir().getAbsolutePath());
        if (initialUrl == null || initialUrl.isEmpty()) {
            initialUrl = "about:blank";
        }
        webView.loadUrl(initialUrl);
        applyKeepScreenOn();
    }

    @Override
    protected void onDestroy() {
        if (webView != null) {
            webView.stopLoading();
            webView.destroy();
            webView = null;
        }
        nativeStop();
        super.onDestroy();
    }

    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed() {
        if (webView != null && webView.canGoBack()) {
            webView.goBack();
        } else {
            super.onBackPressed();
        }
    }

    private void configureWebView(WebView view) {
        WebSettings settings = view.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setDatabaseEnabled(true);
        settings.setMediaPlaybackRequiresUserGesture(false);
        settings.setJavaScriptCanOpenWindowsAutomatically(true);
        settings.setSupportMultipleWindows(false);
        settings.setMixedContentMode(WebSettings.MIXED_CONTENT_ALWAYS_ALLOW);
        settings.setAllowFileAccess(false);
        settings.setAllowContentAccess(false);
        settings.setBuiltInZoomControls(false);
        settings.setDisplayZoomControls(false);
        settings.setLoadsImagesAutomatically(true);

        CookieManager cookieManager = CookieManager.getInstance();
        cookieManager.setAcceptCookie(true);
        cookieManager.setAcceptThirdPartyCookies(view, true);

        view.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView webView, String url) {
                return false;
            }

            @Override
            public void onPageStarted(WebView webView, String url, Bitmap favicon) {
                Log.i(TAG, "page started: " + url);
            }

            @Override
            public void onPageFinished(WebView webView, String url) {
                Log.i(TAG, "page finished: " + url);
                applyKeepScreenOn();

                String script = nativeGetOverlayScript();
                if (script != null && !script.isEmpty()) {
                    webView.evaluateJavascript(script, null);
                }
            }

            @Override
            public void onReceivedError(WebView webView, WebResourceRequest request, WebResourceError error) {
                Log.e(TAG, "page error: " + request.getUrl() + " — " + error.getDescription());
            }

            @Override
            public void onReceivedHttpError(WebView webView, WebResourceRequest request, android.webkit.WebResourceResponse response) {
                Log.e(TAG, "HTTP " + response.getStatusCode() + ": " + request.getUrl());
            }

            @Override
            public void onReceivedSslError(WebView webView, SslErrorHandler handler, SslError error) {
                String host = error.getUrl() == null ? "" : error.getUrl();
                if (nativeAllowInvalidCertificates() || ACCEPTED_CERT_HOSTS.contains(host)) {
                    handler.proceed();
                    return;
                }

                new AlertDialog.Builder(MainActivity.this)
                        .setTitle("Certificate not trusted")
                        .setMessage("The certificate for this server could not be verified.\\n\\n"
                                + "Only continue if this is your own Frigate server and you understand the risk.")
                        .setPositiveButton("Continue anyway", (dialog, which) -> {
                            ACCEPTED_CERT_HOSTS.add(host);
                            handler.proceed();
                        })
                        .setNegativeButton("Cancel", (dialog, which) -> handler.cancel())
                        .setOnCancelListener(dialog -> handler.cancel())
                        .show();
            }
        });

        view.setWebChromeClient(new WebChromeClient() {
            @Override
            public boolean onConsoleMessage(ConsoleMessage message) {
                Log.d(TAG, "console: " + message.message()
                        + " (" + message.sourceId() + ":" + message.lineNumber() + ")");
                return false;
            }
        });

        if ((getApplicationInfo().flags & ApplicationInfo.FLAG_DEBUGGABLE) != 0) {
            WebView.setWebContentsDebuggingEnabled(true);
        }
    }

    private void applyKeepScreenOn() {
        if (nativeKeepScreenOn()) {
            getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        } else {
            getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        }
    }
}
