using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading;
using Windows.Media.Control;
using Windows.Storage.Streams;

namespace MoaMedia
{
    // Bridges Windows' system media controls to Moa over stdio. Every player
    // that shows up in the Windows media flyout works: browsers (YouTube Music,
    // SoundCloud, Yandex Music on the web...), Spotify, the Yandex Music app...
    //
    // stdout: one JSON object per line (hello, state, art, result, error).
    // stdin:  one command per line: "<id>\t<name>\t<arg>". EOF quits.
    internal static class Program
    {
        static readonly object Gate = new object();
        static readonly TextWriter Out = new StreamWriter(Console.OpenStandardOutput(), new UTF8Encoding(false)) { AutoFlush = true };

        static GlobalSystemMediaTransportControlsSessionManager manager;
        static GlobalSystemMediaTransportControlsSession session;
        static string trackKey;
        static string artKey;
        static int artTries;
        static string lastState;
        static DateTime lastSent;
        static int artSeq;

        [MTAThread]
        static int Main(string[] args)
        {
            try
            {
                manager = GlobalSystemMediaTransportControlsSessionManager.RequestAsync().AsTask().Result;
            }
            catch (Exception e)
            {
                Emit("{\"type\":\"error\",\"message\":" + Str(e.GetBaseException().Message) + "}");
                return 1;
            }
            Emit("{\"type\":\"hello\",\"version\":1}");

            bool once = args.Contains("--once");
            if (!once) new Thread(ReadCommands) { IsBackground = true }.Start();

            while (true)
            {
                lock (Gate)
                {
                    try { Poll(); }
                    catch (Exception e) { Emit("{\"type\":\"error\",\"message\":" + Str(e.GetBaseException().Message) + "}"); }
                }
                if (once) return 0;
                Thread.Sleep(400);
            }
        }

        // ------------------------------------------------------------------
        // Reading the current player

        static GlobalSystemMediaTransportControlsSession PickSession()
        {
            var sessions = manager.GetSessions();
            string current = session?.SourceAppUserModelId;
            GlobalSystemMediaTransportControlsSession playing = null;
            foreach (var s in sessions)
            {
                if (!IsPlaying(s)) continue;
                // Stay on the player we already follow while it keeps playing.
                if (s.SourceAppUserModelId == current) return s;
                if (playing == null) playing = s;
            }
            if (playing != null) return playing;
            foreach (var s in sessions)
                if (s.SourceAppUserModelId == current) return s;
            return manager.GetCurrentSession() ?? sessions.FirstOrDefault();
        }

        static bool IsPlaying(GlobalSystemMediaTransportControlsSession s)
        {
            try { return s.GetPlaybackInfo()?.PlaybackStatus == GlobalSystemMediaTransportControlsSessionPlaybackStatus.Playing; }
            catch { return false; }
        }

        static void Poll()
        {
            session = PickSession();
            if (session == null)
            {
                trackKey = artKey = null;
                SendState("{\"type\":\"state\",\"hasTrack\":false,\"playing\":false}", false);
                return;
            }

            GlobalSystemMediaTransportControlsSessionMediaProperties props = null;
            try { props = session.TryGetMediaPropertiesAsync().AsTask().Result; } catch { }
            var info = session.GetPlaybackInfo();
            var tl = session.GetTimelineProperties();
            string app = session.SourceAppUserModelId ?? "";
            string title = props?.Title ?? "";
            string artist = props?.Artist ?? "";
            if (artist == "" && props != null) artist = props.AlbumArtist ?? "";
            bool playing = info?.PlaybackStatus == GlobalSystemMediaTransportControlsSessionPlaybackStatus.Playing;

            double duration = 0, position = 0;
            if (tl != null)
            {
                duration = (tl.EndTime - tl.StartTime).TotalSeconds;
                position = (tl.Position - tl.StartTime).TotalSeconds;
                // The timeline is a snapshot; move it forward while playing.
                if (playing && tl.LastUpdatedTime.Year > 2000) position += (DateTimeOffset.Now - tl.LastUpdatedTime).TotalSeconds;
                if (duration > 0) position = Math.Max(0, Math.Min(position, duration));
                else duration = position = 0;
            }

            string key = app + "\u0001" + title + "\u0001" + artist + "\u0001" + (props?.AlbumTitle ?? "");
            if (key != trackKey)
            {
                trackKey = key;
                artKey = null;
                artTries = 0;
            }
            // Players often publish the cover a moment after the title.
            if (artKey == null && props != null && artTries < 8)
            {
                artTries++;
                string data = ReadThumbnail(props.Thumbnail);
                if (data != null || artTries >= 8)
                {
                    artKey = "art" + (++artSeq);
                    Emit("{\"type\":\"art\",\"key\":" + Str(artKey) + ",\"data\":" + (data == null ? "null" : Str(data)) + "}");
                }
            }

            var c = info?.Controls;
            string repeat = "null";
            if (c != null && c.IsRepeatEnabled && info.AutoRepeatMode.HasValue)
                repeat = Str(info.AutoRepeatMode.Value == Windows.Media.MediaPlaybackAutoRepeatMode.Track ? "ONE"
                    : info.AutoRepeatMode.Value == Windows.Media.MediaPlaybackAutoRepeatMode.List ? "ALL" : "NONE");
            var vol = Volume.Read(app);

            var sb = new StringBuilder("{\"type\":\"state\"");
            sb.Append(",\"hasTrack\":").Append(B(title != ""));
            sb.Append(",\"playing\":").Append(B(playing));
            sb.Append(",\"title\":").Append(Str(title));
            sb.Append(",\"artist\":").Append(Str(artist));
            sb.Append(",\"album\":").Append(Str(props?.AlbumTitle ?? ""));
            sb.Append(",\"app\":").Append(Str(app));
            sb.Append(",\"artwork\":").Append(artKey == null ? "null" : Str(artKey));
            sb.Append(",\"duration\":").Append(N(duration));
            sb.Append(",\"canSeek\":").Append(B(c != null && c.IsPlaybackPositionEnabled));
            sb.Append(",\"canNext\":").Append(B(c == null || c.IsNextEnabled));
            sb.Append(",\"canPrevious\":").Append(B(c == null || c.IsPreviousEnabled));
            sb.Append(",\"repeat\":").Append(repeat);
            sb.Append(",\"volume\":").Append(vol.Found ? N(Math.Round(vol.Value * 100)) : "null");
            sb.Append(",\"muted\":").Append(B(vol.Found && vol.Muted));
            string stable = sb.ToString();
            sb.Append(",\"currentTime\":").Append(N(position)).Append('}');
            SendState(sb.ToString(), playing, stable);
        }

        // Sends on any change, and once a second while playing so the
        // overlay's progress stays in sync.
        static void SendState(string json, bool playing, string stable = null)
        {
            stable = stable ?? json;
            if (stable == lastState && (!playing || (DateTime.UtcNow - lastSent).TotalMilliseconds < 1000)) return;
            lastState = stable;
            lastSent = DateTime.UtcNow;
            Emit(json);
        }

        static string ReadThumbnail(IRandomAccessStreamReference thumb)
        {
            if (thumb == null) return null;
            try
            {
                using (var stream = thumb.OpenReadAsync().AsTask().Result)
                {
                    if (stream == null || stream.Size == 0 || stream.Size > 4_000_000) return null;
                    var bytes = new byte[stream.Size];
                    using (var reader = new DataReader(stream.GetInputStreamAt(0)))
                    {
                        reader.LoadAsync((uint)stream.Size).AsTask().Wait();
                        reader.ReadBytes(bytes);
                    }
                    string type = string.IsNullOrEmpty(stream.ContentType) ? "image/png" : stream.ContentType;
                    return "data:" + type + ";base64," + Convert.ToBase64String(bytes);
                }
            }
            catch
            {
                return null;
            }
        }

        // ------------------------------------------------------------------
        // Commands

        static void ReadCommands()
        {
            string line;
            while ((line = Console.In.ReadLine()) != null)
            {
                var parts = line.Split('\t');
                string id = parts[0];
                string name = parts.Length > 1 ? parts[1] : "";
                string arg = parts.Length > 2 ? parts[2] : "";
                string value = "null";
                try
                {
                    lock (Gate) value = Run(name, arg);
                }
                catch { }
                Emit("{\"type\":\"result\",\"id\":" + Str(id) + ",\"value\":" + value + "}");
            }
            Environment.Exit(0);
        }

        static string Run(string name, string arg)
        {
            double.TryParse(arg, NumberStyles.Float, CultureInfo.InvariantCulture, out double num);
            var s = session ?? PickSession();
            string app = s?.SourceAppUserModelId ?? "";
            switch (name)
            {
                case "ping": return "true";
                case "volumeBy": return Volume.Change(app, num / 100, null);
                case "setVolume": return Volume.Change(app, null, num / 100);
                case "toggleMute": return Volume.ToggleMute(app);
            }
            if (s == null) return "null";
            switch (name)
            {
                case "playPause": return B(Ok(s.TryTogglePlayPauseAsync()));
                case "play": return B(Ok(s.TryPlayAsync()));
                case "pause": return B(Ok(s.TryPauseAsync()));
                case "next": return B(Ok(s.TrySkipNextAsync()));
                case "previous": return B(Ok(s.TrySkipPreviousAsync()));
                case "seekTo": return B(SeekTo(s, num));
                case "seekBy":
                {
                    var tl = s.GetTimelineProperties();
                    double pos = (tl.Position - tl.StartTime).TotalSeconds;
                    if (s.GetPlaybackInfo()?.PlaybackStatus == GlobalSystemMediaTransportControlsSessionPlaybackStatus.Playing && tl.LastUpdatedTime.Year > 2000)
                        pos += (DateTimeOffset.Now - tl.LastUpdatedTime).TotalSeconds;
                    return B(SeekTo(s, Math.Max(0, pos + num)));
                }
                case "cycleRepeat":
                {
                    var info = s.GetPlaybackInfo();
                    if (info?.Controls == null || !info.Controls.IsRepeatEnabled) return Str("unsupported");
                    var mode = info.AutoRepeatMode ?? Windows.Media.MediaPlaybackAutoRepeatMode.None;
                    var next = mode == Windows.Media.MediaPlaybackAutoRepeatMode.None ? Windows.Media.MediaPlaybackAutoRepeatMode.List
                        : mode == Windows.Media.MediaPlaybackAutoRepeatMode.List ? Windows.Media.MediaPlaybackAutoRepeatMode.Track
                        : Windows.Media.MediaPlaybackAutoRepeatMode.None;
                    if (!Ok(s.TryChangeAutoRepeatModeAsync(next))) return Str("unsupported");
                    return Str(next == Windows.Media.MediaPlaybackAutoRepeatMode.Track ? "ONE" : next == Windows.Media.MediaPlaybackAutoRepeatMode.List ? "ALL" : "NONE");
                }
            }
            return "null";
        }

        static bool SeekTo(GlobalSystemMediaTransportControlsSession s, double seconds)
        {
            var tl = s.GetTimelineProperties();
            var target = tl.StartTime + TimeSpan.FromSeconds(Math.Max(0, seconds));
            if (tl.EndTime > tl.StartTime && target > tl.EndTime) target = tl.EndTime;
            return Ok(s.TryChangePlaybackPositionAsync(target.Ticks));
        }

        static bool Ok(Windows.Foundation.IAsyncOperation<bool> op)
        {
            try { return op.AsTask().Wait(2000) && op.GetResults(); }
            catch { return false; }
        }

        // ------------------------------------------------------------------
        // JSON helpers

        static void Emit(string line)
        {
            lock (Out) Out.WriteLine(line);
        }

        static string B(bool v) => v ? "true" : "false";

        static string N(double v) => double.IsNaN(v) || double.IsInfinity(v) ? "0" : v.ToString("0.###", CultureInfo.InvariantCulture);

        static string Str(string s)
        {
            var sb = new StringBuilder("\"");
            foreach (char ch in s ?? "")
            {
                switch (ch)
                {
                    case '"': sb.Append("\\\""); break;
                    case '\\': sb.Append("\\\\"); break;
                    case '\n': sb.Append("\\n"); break;
                    case '\r': sb.Append("\\r"); break;
                    case '\t': sb.Append("\\t"); break;
                    default:
                        if (ch < 0x20) sb.Append("\\u").Append(((int)ch).ToString("x4"));
                        else sb.Append(ch);
                        break;
                }
            }
            return sb.Append('"').ToString();
        }
    }
}
