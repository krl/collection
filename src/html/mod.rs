use Val;
use stash::RelStash;
use meta::Meta;

pub const CSS: &'static str = "
  .lbranch { display: inline-block; float: right; }
  .mid { background: #ccc;
         padding: 3px;
         border-radius: 4px;
         text-align: center; }
  .stash { border: 1px solid black; }
  .line { border: 1px solid black;
          border-radius: 4px; padding: 4px;
          margin-bottom: 4px; }
  .hilight-true {
    background: #eee;
  }
  .leaf { display: inline-block;
          margin: 1px;
          border-radius: 4px;
          background: #ddd;
          font-size: small; }
  .rel { display: inline-block;
         border-radius: 5px; }
  .loc { margin-right: 4px;
         display: inline-block;
         border-radius: 5px; }
  .col-1 { background: #faa; }
  .col-2 { background: #afa; }
  .col-3 { background: #aaf; }
  .col-4 { background: #aff; }
  .col-5 { background: #ffa; }
  .col-6 { background: #faf; }
  .weight-1 { background: red; }
  .weight-2 { background: green; }
  .weight-3 { background: blue; }
  .weight-4 { background: teal; }
  .weight-5 { background: yellow; }
  .weight-6 { background: purple; }
  .weight-7 { background: brown; }
  .weight-8 { background: orange; }
  .node { display: inline-block;
          margin: 1px;
          padding: 1px;
          border-radius: 4px;
          border-top: 1px solid black;
          border-bottom: 2px solid black;
          border-left: 1px solid black;
          border-right: 2px solid black;
        }
  .leaf { display: inline-block;
          min-width: 3px;
          min-height: 3px;
          margin: 1px;
          padding: 1px;
          border-radius: 4px;
          border-bottom: 2px solid black;
          border-right: 1px solid black;
        }
";

pub trait Html<T, M>
    where T: Val,
          M: Meta<T>
{
    fn _html(&self, stash: RelStash<T, M>) -> String;
}
