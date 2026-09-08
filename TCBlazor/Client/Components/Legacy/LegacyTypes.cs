namespace TCBlazor.Client.Components
{
    /// <summary>
    /// The handful of AntDesign constants the classic markup names, kept so that markup
    /// does not have to change when the package goes. The values are our own class names
    /// rather than Ant's, because that is what dresses them now.
    /// </summary>
    public static class ButtonType
    {
        public const string Primary = "primary";
        public const string Default = "default";
        public const string Dashed = "dashed";
        public const string Link = "link";
        public const string Text = "text";
    }

    public static class ButtonSize
    {
        public const string Small = "small";
        public const string Large = "large";
        public const string Default = "default";
    }

    public static class InputSize
    {
        public const string Small = "small";
        public const string Large = "large";
        public const string Default = "default";
    }

    public static class InputType
    {
        public const string Text = "text";
        public const string Number = "number";
        public const string Password = "password";
    }

    public static class ButtonShape
    {
        public const string Circle = "circle";
        public const string Round = "round";
        public const string Default = "";
    }

    /// <summary>
    /// Ant named its icons through a nested type; only "close" is ever asked for, and
    /// TcIcon already draws it.
    /// </summary>
    public static class IconType
    {
        public static class Outline
        {
            public const string Close = "close";
        }
    }
}
