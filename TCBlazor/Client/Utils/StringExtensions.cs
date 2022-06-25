namespace TCBlazor.Client.Utils
{
    public static class StringExtensions
    {
        public static string AsBreakable(this string str, int chunkSize = 6)
        {
            var safeStr = str.Replace("<", "&lt;");
            var prepared = string.Join("<wbr/>", ChunksUpto(safeStr, chunkSize));
            prepared = prepared.Replace("\r\n", "\n").Replace("\n", "<br/>");
            return prepared;
        }
        public static IEnumerable<string> ChunksUpto(string str, int maxChunkSize)
        {
            for (int i = 0; i < str.Length; i += maxChunkSize)
                yield return str.Substring(i, Math.Min(maxChunkSize, str.Length - i));
        }
    }
}
