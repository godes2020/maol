using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.Runtime.InteropServices;

namespace MoaMedia
{
    // Volume of the player app in the Windows volume mixer (Core Audio
    // sessions). Media controls have no volume, so this is how Moa turns the
    // music up or down without touching the game or the system volume.
    internal static class Volume
    {
        internal struct Level
        {
            public bool Found;
            public float Value;
            public bool Muted;
        }

        // Processes that play music and publish media controls.
        static readonly HashSet<string> Players = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
        {
            "chrome", "msedge", "firefox", "zen", "opera", "brave", "browser", "vivaldi", "librewolf",
            "floorp", "waterfox", "arc", "chromium", "yandex", "spotify", "yandexmusic", "yandex music",
            "яндекс музыка", "soundcloud", "deezer", "tidal", "applemusic", "amazon music", "vk", "vk music"
        };

        static readonly Dictionary<uint, string> Names = new Dictionary<uint, string>();
        static DateTime cacheAt;
        static string cacheApp;
        static Level cache;

        public static Level Read(string app)
        {
            // Reading the mixer is cheap but not free; the overlay only needs it now and then.
            if (app == cacheApp && (DateTime.UtcNow - cacheAt).TotalMilliseconds < 1500) return cache;
            var level = new Level();
            var vols = Find(app);
            if (vols.Count > 0)
            {
                level.Found = true;
                vols[0].GetMasterVolume(out level.Value);
                vols[0].GetMute(out level.Muted);
            }
            Release(vols);
            cacheApp = app;
            cacheAt = DateTime.UtcNow;
            cache = level;
            return level;
        }

        public static string Change(string app, double? delta, double? absolute)
        {
            var vols = Find(app);
            try
            {
                if (vols.Count == 0) return "null";
                vols[0].GetMasterVolume(out float cur);
                float next = (float)Math.Max(0, Math.Min(1, absolute ?? (cur + (delta ?? 0))));
                var ctx = Guid.Empty;
                foreach (var v in vols)
                {
                    v.SetMasterVolume(next, ref ctx);
                    if (next > 0) v.SetMute(false, ref ctx);
                }
                cacheAt = DateTime.MinValue;
                return Math.Round(next * 100).ToString(CultureInfo.InvariantCulture);
            }
            finally
            {
                Release(vols);
            }
        }

        public static string ToggleMute(string app)
        {
            var vols = Find(app);
            try
            {
                if (vols.Count == 0) return "null";
                vols[0].GetMute(out bool muted);
                var ctx = Guid.Empty;
                foreach (var v in vols) v.SetMute(!muted, ref ctx);
                cacheAt = DateTime.MinValue;
                return !muted ? "true" : "false";
            }
            finally
            {
                Release(vols);
            }
        }

        // Finds the mixer sessions that belong to the player with this
        // AppUserModelID ("Chrome", "MSEdge", "Spotify.exe", a hash for Firefox...).
        static List<ISimpleAudioVolume> Find(string app)
        {
            var all = new List<(ISimpleAudioVolume vol, string name, bool active)>();
            try
            {
                var enumerator = (IMMDeviceEnumerator)new MMDeviceEnumerator();
                if (enumerator.GetDefaultAudioEndpoint(0 /* eRender */, 1 /* eMultimedia */, out IMMDevice device) != 0) return new List<ISimpleAudioVolume>();
                var iid = typeof(IAudioSessionManager2).GUID;
                if (device.Activate(ref iid, 23 /* CLSCTX_ALL */, IntPtr.Zero, out object o) != 0) return new List<ISimpleAudioVolume>();
                var mgr = (IAudioSessionManager2)o;
                if (mgr.GetSessionEnumerator(out IAudioSessionEnumerator list) != 0) return new List<ISimpleAudioVolume>();
                list.GetCount(out int count);
                for (int i = 0; i < count; i++)
                {
                    if (list.GetSession(i, out IAudioSessionControl2 ctl) != 0 || ctl == null) continue;
                    ctl.GetProcessId(out uint pid);
                    ctl.GetState(out int state);
                    string name = ProcessName(pid);
                    if (pid == 0 || name == "")
                    {
                        Marshal.ReleaseComObject(ctl);
                        continue;
                    }
                    all.Add(((ISimpleAudioVolume)ctl, name, state == 1));
                }
                Marshal.ReleaseComObject(list);
                Marshal.ReleaseComObject(mgr);
                Marshal.ReleaseComObject(device);
                Marshal.ReleaseComObject(enumerator);
            }
            catch
            {
                // No audio device or the mixer is busy; volume just isn't available.
            }

            string a = (app ?? "").ToLowerInvariant().Replace(".exe", "");
            var picked = all.Where(s => a != "" && (a.Contains(s.name) || s.name.Contains(a))).ToList();
            // Firefox-based browsers publish a hash as their id; fall back to a
            // known player that is producing sound right now.
            if (picked.Count == 0) picked = all.Where(s => s.active && Players.Contains(s.name)).ToList();
            if (picked.Count == 0) picked = all.Where(s => Players.Contains(s.name)).ToList();

            foreach (var s in all)
                if (!picked.Contains(s)) Marshal.ReleaseComObject(s.vol);
            return picked.Select(s => s.vol).ToList();
        }

        static void Release(List<ISimpleAudioVolume> vols)
        {
            foreach (var v in vols)
            {
                try { Marshal.ReleaseComObject(v); } catch { }
            }
        }

        static string ProcessName(uint pid)
        {
            if (Names.TryGetValue(pid, out string n)) return n;
            try
            {
                using (var p = Process.GetProcessById((int)pid)) n = p.ProcessName.ToLowerInvariant();
            }
            catch
            {
                n = "";
            }
            if (Names.Count > 500) Names.Clear();
            Names[pid] = n;
            return n;
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

        [ComImport, Guid("77AA99A0-1BD6-484F-8BC7-2C654C9A9B6F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IAudioSessionManager2
        {
            [PreserveSig] int GetAudioSessionControl(IntPtr sessionGuid, int flags, out IntPtr control);
            [PreserveSig] int GetSimpleAudioVolume(IntPtr sessionGuid, int flags, out IntPtr volume);
            [PreserveSig] int GetSessionEnumerator(out IAudioSessionEnumerator sessions);
        }

        [ComImport, Guid("E2F5BB11-0570-40CA-ACDD-3AA01277DEE8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IAudioSessionEnumerator
        {
            [PreserveSig] int GetCount(out int count);
            [PreserveSig] int GetSession(int index, out IAudioSessionControl2 session);
        }

        [ComImport, Guid("bfb7ff88-7239-4fc9-8fa2-07c950be9c6d"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IAudioSessionControl2
        {
            [PreserveSig] int GetState(out int state);
            [PreserveSig] int GetDisplayName([MarshalAs(UnmanagedType.LPWStr)] out string name);
            [PreserveSig] int SetDisplayName([MarshalAs(UnmanagedType.LPWStr)] string name, ref Guid ctx);
            [PreserveSig] int GetIconPath([MarshalAs(UnmanagedType.LPWStr)] out string path);
            [PreserveSig] int SetIconPath([MarshalAs(UnmanagedType.LPWStr)] string path, ref Guid ctx);
            [PreserveSig] int GetGroupingParam(out Guid param);
            [PreserveSig] int SetGroupingParam(ref Guid param, ref Guid ctx);
            [PreserveSig] int RegisterAudioSessionNotification(IntPtr client);
            [PreserveSig] int UnregisterAudioSessionNotification(IntPtr client);
            [PreserveSig] int GetSessionIdentifier([MarshalAs(UnmanagedType.LPWStr)] out string id);
            [PreserveSig] int GetSessionInstanceIdentifier([MarshalAs(UnmanagedType.LPWStr)] out string id);
            [PreserveSig] int GetProcessId(out uint pid);
            [PreserveSig] int IsSystemSoundsSession();
            [PreserveSig] int SetDuckingPreference(bool optOut);
        }

        [ComImport, Guid("87CE5498-68D6-44E5-9215-6DA47EF883D8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface ISimpleAudioVolume
        {
            [PreserveSig] int SetMasterVolume(float level, ref Guid ctx);
            [PreserveSig] int GetMasterVolume(out float level);
            [PreserveSig] int SetMute(bool mute, ref Guid ctx);
            [PreserveSig] int GetMute(out bool muted);
        }
    }
}
