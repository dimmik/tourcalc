using TCalcCore.Storage;

namespace TCBlazor.Server.Pages
{
    public class tcls : ITourcalcLocalStorage
    {
        public Task<(string val, DateTimeOffset stored)> Get(string key, string defVal = "", bool storeDefaultValue = false)
        {
            if (key == "__tc_token") return Task.FromResult(("eyJhbGciOiJFUzI1NiIsInR5cCI6IkpXVCJ9.eyJodHRwOi8vc2NoZW1hcy54bWxzb2FwLm9yZy93cy8yMDA1LzA1L2lkZW50aXR5L2NsYWltcy9uYW1laWRlbnRpZmllciI6ImNvZGUiLCJBdXRoRGF0YUpzb24iOiJ7XCJUeXBlXCI6XCJBY2Nlc3NDb2RlXCIsXCJJc01hc3RlclwiOmZhbHNlLFwiQWNjZXNzQ29kZU1ENVwiOlwiMUMwMzY5RUE0MkIyRjc0NkQzREYxRTY2QkNCMkRFNDZcIn0iLCJleHAiOjE2NzMxNjQyNjksImlzcyI6IlRvdXJDYWxjIiwiYXVkIjoiVXNlcnMifQ.c_lpdgUqw7b6Cm2YDXrUaCfo_hJ8nIhdhalTPiq3iSbwKdlZz03aJDY1nR4cTEYrVB9HwDXc2D2tVCoacD4f6w", DateTimeOffset.Now));
            return Task.FromResult((defVal, DateTimeOffset.Now));
        }

        public Task Set(string key, string val)
        {
            // nothing
            return Task.CompletedTask;
        }
    }
}
