//! 再ログインの判断（`cli::relogin::open_with_relogin`）の単体テスト。
//! ブラウザの代わりに偽の関数を渡す（Chrome は開かない）。

use std::cell::{Cell, RefCell};

use edictora::cli::relogin::open_with_relogin;
use edictora::error::{Error, Result};

/// 呼び出しの記録つきで、ensure の結果を順に返す偽の環境
struct Fake {
    log: RefCell<Vec<&'static str>>,
    ensure_results: RefCell<Vec<Result<()>>>,
    relogin_result: RefCell<Option<Result<()>>>,
    reported: Cell<u32>,
}

impl Fake {
    fn new(ensure_results: Vec<Result<()>>, relogin: Result<()>) -> Self {
        Fake {
            log: RefCell::new(Vec::new()),
            ensure_results: RefCell::new(ensure_results),
            relogin_result: RefCell::new(Some(relogin)),
            reported: Cell::new(0),
        }
    }

    fn run(&self) -> Result<u32> {
        let opened = Cell::new(0u32);
        open_with_relogin(
            || {
                self.log.borrow_mut().push("open");
                opened.set(opened.get() + 1);
                Ok(opened.get())
            },
            |_| {
                self.log.borrow_mut().push("ensure");
                self.ensure_results.borrow_mut().remove(0)
            },
            || {
                self.log.borrow_mut().push("relogin");
                self.relogin_result
                    .borrow_mut()
                    .take()
                    .expect("relogin は 1 回しか呼ばれないはず")
            },
            |_| self.reported.set(self.reported.get() + 1),
        )
    }

    fn calls(&self) -> Vec<&'static str> {
        self.log.borrow().clone()
    }
}

#[test]
fn ログイン済みならログイン手順を呼ばない() {
    let fake = Fake::new(vec![Ok(())], Ok(()));
    assert_eq!(fake.run().unwrap(), 1);
    assert_eq!(fake.calls(), ["open", "ensure"]);
}

#[test]
fn ログインが要るとログイン手順の後に開き直して続ける() {
    let fake = Fake::new(vec![Err(Error::LoginRequired), Ok(())], Ok(()));
    // 2 回目に開いたブラウザが返る
    assert_eq!(fake.run().unwrap(), 2);
    assert_eq!(
        fake.calls(),
        ["open", "ensure", "relogin", "open", "ensure"]
    );
    assert_eq!(fake.reported.get(), 0);
}

#[test]
fn 再試行は1回だけで2回目もログイン画面ならログイン要のまま() {
    let fake = Fake::new(
        vec![Err(Error::LoginRequired), Err(Error::LoginRequired)],
        Ok(()),
    );
    assert!(matches!(fake.run(), Err(Error::LoginRequired)));
    assert_eq!(
        fake.calls(),
        ["open", "ensure", "relogin", "open", "ensure"]
    );
}

#[test]
fn ログイン手順が失敗したら理由を報告してログイン要で終える() {
    let fake = Fake::new(
        vec![Err(Error::LoginRequired)],
        Err(Error::Dashboard("待ち切れませんでした".to_owned())),
    );
    assert!(matches!(fake.run(), Err(Error::LoginRequired)));
    // 開き直さない
    assert_eq!(fake.calls(), ["open", "ensure", "relogin"]);
    assert_eq!(fake.reported.get(), 1);
}

#[test]
fn ログイン手順の中断はそのまま伝わり報告しない() {
    let fake = Fake::new(vec![Err(Error::LoginRequired)], Err(Error::Cancelled));
    assert!(matches!(fake.run(), Err(Error::Cancelled)));
    assert_eq!(fake.calls(), ["open", "ensure", "relogin"]);
    assert_eq!(fake.reported.get(), 0);
}

#[test]
fn 確認中の中断とログイン要以外のエラーではログイン手順を呼ばない() {
    let fake = Fake::new(vec![Err(Error::Cancelled)], Ok(()));
    assert!(matches!(fake.run(), Err(Error::Cancelled)));
    assert_eq!(fake.calls(), ["open", "ensure"]);

    let fake = Fake::new(vec![Err(Error::Dashboard("x".to_owned()))], Ok(()));
    assert!(matches!(fake.run(), Err(Error::Dashboard(_))));
    assert_eq!(fake.calls(), ["open", "ensure"]);
}

#[test]
fn 開き直した後の確認で中断されてもそのまま伝わる() {
    let fake = Fake::new(
        vec![Err(Error::LoginRequired), Err(Error::Cancelled)],
        Ok(()),
    );
    assert!(matches!(fake.run(), Err(Error::Cancelled)));
    assert_eq!(
        fake.calls(),
        ["open", "ensure", "relogin", "open", "ensure"]
    );
}
