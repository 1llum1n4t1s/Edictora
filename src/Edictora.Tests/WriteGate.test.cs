using Edictora.Core.Dashboard;

namespace Edictora.Tests;

[TestClass]
public sealed class WriteGateTests
{
    private static ItemPageState State(string? status, bool descEditable = true, bool saveFound = true, bool saveEnabled = true)
        => new(status, true, descEditable, saveFound, saveEnabled);

    [TestMethod]
    public void Evaluate_確認済みの編集可能な状態なら許可する()
    {
        Assert.IsTrue(WriteGate.Evaluate(State("公開済み")).Allowed);
        Assert.IsTrue(WriteGate.Evaluate(State("下書き")).Allowed);
    }

    [TestMethod]
    public void Evaluate_審査待ちは止める()
    {
        var decision = WriteGate.Evaluate(State("審査待ち"));

        Assert.IsFalse(decision.Allowed);
        StringAssert.Contains(decision.Reason, "審査");
    }

    [TestMethod]
    public void Evaluate_ステータスが読めないか未知なら安全側に倒す()
    {
        Assert.IsFalse(WriteGate.Evaluate(State(null)).Allowed);
        Assert.IsFalse(WriteGate.Evaluate(State("  ")).Allowed);
        Assert.IsFalse(WriteGate.Evaluate(State("取り下げ")).Allowed);
    }

    [TestMethod]
    public void Evaluate_審査中の語を含めば編集可能な語があっても止める()
    {
        Assert.IsFalse(WriteGate.Evaluate(State("公開済み（更新は審査待ち）")).Allowed);
    }

    [TestMethod]
    public void Evaluate_欄や保存ボタンが使えなければ止める()
    {
        Assert.IsFalse(WriteGate.Evaluate(State("公開済み", descEditable: false)).Allowed);
        Assert.IsFalse(WriteGate.Evaluate(State("公開済み", saveFound: false)).Allowed);
        Assert.IsFalse(WriteGate.Evaluate(State("公開済み", saveEnabled: false)).Allowed);
        Assert.IsFalse(WriteGate.Evaluate(new ItemPageState("公開済み", false, false, true, true)).Allowed);
    }

    [TestMethod]
    public void EnsureAllowed_不可ならWriteBlockedException()
    {
        Assert.ThrowsExactly<WriteBlockedException>(() => WriteGate.EnsureAllowed(State("審査待ち")));
        WriteGate.EnsureAllowed(State("公開済み"));
    }
}
