using System;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

namespace MoaMedia
{
    // Beat analysis for the overlay glow. Listens to what Windows plays
    // (WASAPI loopback on the default output) and sends ~30 small frames a
    // second: energy, bass, five bands, beat and tempo. Runs only while Moa
    // asks for it (music playing and the glow turned on).
    internal static class Audio
    {
        const int N = 1024;
        static Thread thread;
        static volatile bool wanted;

        public static void Enable(bool on)
        {
            wanted = on;
            if (on && thread == null)
            {
                thread = new Thread(Run) { IsBackground = true, Priority = ThreadPriority.BelowNormal };
                thread.Start();
            }
        }

        static void Run()
        {
            while (true)
            {
                if (!wanted)
                {
                    Thread.Sleep(250);
                    continue;
                }
                try { Capture(); }
                catch { }
                // Device changed or unavailable: try again shortly.
                Thread.Sleep(1500);
            }
        }

        static void Capture()
        {
            var enumerator = (IMMDeviceEnumerator)new MMDeviceEnumerator();
            IMMDevice device = null;
            IAudioClient client = null;
            IAudioCaptureClient capture = null;
            IntPtr format = IntPtr.Zero;
            try
            {
                Check(enumerator.GetDefaultAudioEndpoint(0 /* eRender */, 0 /* eConsole */, out device));
                var iid = typeof(IAudioClient).GUID;
                Check(device.Activate(ref iid, 23, IntPtr.Zero, out object o));
                client = (IAudioClient)o;
                Check(client.GetMixFormat(out format));
                int channels = Marshal.ReadInt16(format, 2);
                int rate = Marshal.ReadInt32(format, 4);
                int bits = Marshal.ReadInt16(format, 14);
                Check(client.Initialize(0 /* shared */, 0x00020000 /* loopback */, 2_000_000, 0, format, IntPtr.Zero));
                var ciid = typeof(IAudioCaptureClient).GUID;
                Check(client.GetService(ref ciid, out object c));
                capture = (IAudioCaptureClient)c;
                Check(client.Start());

                var analyzer = new Analyzer(rate);
                var ring = new float[N];
                int ringPos = 0;
                var buf = new byte[0];
                var lastFrame = DateTime.UtcNow;
                bool sentSilence = false;

                while (wanted)
                {
                    Thread.Sleep(10);
                    while (true)
                    {
                        Check(capture.GetNextPacketSize(out uint packet));
                        if (packet == 0) break;
                        Check(capture.GetBuffer(out IntPtr data, out uint frames, out uint flags, out ulong _, out ulong __));
                        int bytes = (int)frames * channels * bits / 8;
                        if (buf.Length < bytes) buf = new byte[bytes];
                        bool silent = (flags & 0x2) != 0;
                        if (!silent) Marshal.Copy(data, buf, 0, bytes);
                        Check(capture.ReleaseBuffer(frames));
                        for (int f = 0; f < frames; f++)
                        {
                            float sum = 0;
                            if (!silent)
                            {
                                for (int ch = 0; ch < channels; ch++)
                                {
                                    int i = (f * channels + ch) * bits / 8;
                                    sum += bits == 32 ? BitConverter.ToSingle(buf, i) : bits == 16 ? BitConverter.ToInt16(buf, i) / 32768f : 0;
                                }
                            }
                            ring[ringPos] = sum / channels;
                            ringPos = (ringPos + 1) % N;
                        }
                    }

                    if ((DateTime.UtcNow - lastFrame).TotalMilliseconds < 33) continue;
                    lastFrame = DateTime.UtcNow;
                    string json = analyzer.Frame(ring, ringPos);
                    if (json == null)
                    {
                        if (sentSilence) continue;
                        sentSilence = true;
                        json = "{\"type\":\"audio\",\"energy\":0,\"bass\":0,\"beat\":false,\"bands\":[0,0,0,0,0],\"bpm\":0}";
                    }
                    else
                    {
                        sentSilence = false;
                    }
                    Program.Emit(json);
                }
                client.Stop();
            }
            finally
            {
                if (format != IntPtr.Zero) Marshal.FreeCoTaskMem(format);
                if (capture != null) Marshal.ReleaseComObject(capture);
                if (client != null) Marshal.ReleaseComObject(client);
                if (device != null) Marshal.ReleaseComObject(device);
                Marshal.ReleaseComObject(enumerator);
            }
        }

        static void Check(int hr)
        {
            if (hr < 0) throw new COMException("audio", hr);
        }

        // Mirrors the Web Audio analysis used by the browser bridge so the
        // glow looks the same with every source.
        sealed class Analyzer
        {
            readonly int rate;
            readonly double[] re = new double[N];
            readonly double[] im = new double[N];
            readonly double[] smooth = new double[N / 2];
            readonly double[] window = new double[N];
            double bassAvg;
            DateTime lastBeat = DateTime.MinValue;
            readonly double[] gaps = new double[12];
            int gapCount, gapPos;

            public Analyzer(int rate)
            {
                this.rate = rate;
                for (int i = 0; i < N; i++) window[i] = 0.5 * (1 - Math.Cos(2 * Math.PI * i / (N - 1)));
            }

            public string Frame(float[] ring, int pos)
            {
                double peak = 0;
                for (int i = 0; i < N; i++)
                {
                    double v = ring[(pos + i) % N];
                    peak = Math.Max(peak, Math.Abs(v));
                    re[i] = v * window[i];
                    im[i] = 0;
                }
                if (peak < 1e-4) return null;
                Fft(re, im);
                for (int k = 0; k < N / 2; k++)
                {
                    double mag = Math.Sqrt(re[k] * re[k] + im[k] * im[k]) / N;
                    smooth[k] = 0.6 * smooth[k] + 0.4 * mag;
                }

                double binHz = (double)rate / N;
                Func<double, int> bin = hz => Math.Max(1, (int)Math.Round(hz / binHz));
                Func<int, int, double> avg = (from, to) =>
                {
                    double s = 0;
                    for (int k = from; k < to; k++)
                    {
                        // Same scale as AnalyserNode.getByteFrequencyData (-100..-30 dB).
                        double db = 20 * Math.Log10(smooth[k] + 1e-12);
                        s += Math.Max(0, Math.Min(1, (db + 100) / 70));
                    }
                    return s / Math.Max(1, to - from);
                };

                double bass = avg(bin(40), bin(160));
                double energy = avg(bin(40), bin(8000));
                int[] edges = { 40, 160, 500, 1500, 4000, 10000 };
                var bands = new double[5];
                for (int i = 0; i < 5; i++) bands[i] = avg(bin(edges[i]), bin(edges[i + 1]));

                var now = DateTime.UtcNow;
                double since = (now - lastBeat).TotalMilliseconds;
                bool beat = false;
                if (bass > bassAvg * 1.25 && bass > 0.3 && since > 280)
                {
                    beat = true;
                    if (since < 1500)
                    {
                        gaps[gapPos] = since;
                        gapPos = (gapPos + 1) % gaps.Length;
                        gapCount = Math.Min(gapCount + 1, gaps.Length);
                    }
                    lastBeat = now;
                }
                bassAvg = bassAvg * 0.92 + bass * 0.08;
                int bpm = 0;
                if (gapCount >= 4)
                {
                    var g = new double[gapCount];
                    Array.Copy(gaps, g, gapCount);
                    Array.Sort(g);
                    bpm = (int)Math.Round(60000 / g[gapCount / 2]);
                }

                var sb = new StringBuilder("{\"type\":\"audio\"");
                sb.Append(",\"energy\":").Append(F(energy));
                sb.Append(",\"bass\":").Append(F(bass));
                sb.Append(",\"beat\":").Append(beat ? "true" : "false");
                sb.Append(",\"bands\":[");
                for (int i = 0; i < 5; i++) sb.Append(i > 0 ? "," : "").Append(F(bands[i]));
                sb.Append("],\"bpm\":").Append(bpm).Append('}');
                return sb.ToString();
            }

            static string F(double v) => v.ToString("0.###", CultureInfo.InvariantCulture);

            static void Fft(double[] re, double[] im)
            {
                int n = re.Length;
                for (int i = 1, j = 0; i < n; i++)
                {
                    int bit = n >> 1;
                    for (; (j & bit) != 0; bit >>= 1) j ^= bit;
                    j ^= bit;
                    if (i < j)
                    {
                        double t = re[i]; re[i] = re[j]; re[j] = t;
                        t = im[i]; im[i] = im[j]; im[j] = t;
                    }
                }
                for (int len = 2; len <= n; len <<= 1)
                {
                    double ang = -2 * Math.PI / len;
                    double wr = Math.Cos(ang), wi = Math.Sin(ang);
                    for (int i = 0; i < n; i += len)
                    {
                        double cr = 1, ci = 0;
                        for (int k = 0; k < len / 2; k++)
                        {
                            int a = i + k, b = i + k + len / 2;
                            double xr = re[b] * cr - im[b] * ci;
                            double xi = re[b] * ci + im[b] * cr;
                            re[b] = re[a] - xr; im[b] = im[a] - xi;
                            re[a] += xr; im[a] += xi;
                            double nr = cr * wr - ci * wi;
                            ci = cr * wi + ci * wr;
                            cr = nr;
                        }
                    }
                }
            }
        }

        // ---- Core Audio COM interfaces ----

        [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")]
        class MMDeviceEnumerator { }

        [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IMMDeviceEnumerator
        {
            [PreserveSig] int EnumAudioEndpoints(int dataFlow, int stateMask, out IntPtr devices);
            [PreserveSig] int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice device);
        }

        [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IMMDevice
        {
            [PreserveSig] int Activate(ref Guid iid, int clsCtx, IntPtr activationParams, [MarshalAs(UnmanagedType.IUnknown)] out object instance);
        }

        [ComImport, Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IAudioClient
        {
            [PreserveSig] int Initialize(int shareMode, int streamFlags, long bufferDuration, long periodicity, IntPtr format, IntPtr sessionGuid);
            [PreserveSig] int GetBufferSize(out uint frames);
            [PreserveSig] int GetStreamLatency(out long latency);
            [PreserveSig] int GetCurrentPadding(out uint padding);
            [PreserveSig] int IsFormatSupported(int shareMode, IntPtr format, out IntPtr closest);
            [PreserveSig] int GetMixFormat(out IntPtr format);
            [PreserveSig] int GetDevicePeriod(out long defaultPeriod, out long minimumPeriod);
            [PreserveSig] int Start();
            [PreserveSig] int Stop();
            [PreserveSig] int Reset();
            [PreserveSig] int SetEventHandle(IntPtr handle);
            [PreserveSig] int GetService(ref Guid iid, [MarshalAs(UnmanagedType.IUnknown)] out object service);
        }

        [ComImport, Guid("C8ADBD64-E71E-48a0-A4DE-185C395CD317"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IAudioCaptureClient
        {
            [PreserveSig] int GetBuffer(out IntPtr data, out uint frames, out uint flags, out ulong devicePosition, out ulong qpcPosition);
            [PreserveSig] int ReleaseBuffer(uint frames);
            [PreserveSig] int GetNextPacketSize(out uint frames);
        }
    }
}
