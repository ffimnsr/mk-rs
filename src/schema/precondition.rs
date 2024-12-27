use serde::{Deserialize, Serialize};


#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Precondition {
  pub command: String,
  pub message: Option<String>,
}

mod test {
  use super::*;

  #[test]
  fn test_precondition() {
    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        message: 'This is a message'
      ";
      let precondition = serde_yaml::from_str::<Precondition>(yaml).unwrap();

      assert_eq!(precondition.command, "echo \"Hello, World!\"");
      assert_eq!(precondition.message, Some("This is a message".into()));
    }

    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
      ";
      let precondition = serde_yaml::from_str::<Precondition>(yaml).unwrap();

      assert_eq!(precondition.command, "echo \"Hello, World!\"");
      assert_eq!(precondition.message, None);
    }

    {
      let yaml = "
        command: 'echo \"Hello, World!\"'
        message: null
      ";
      let precondition = serde_yaml::from_str::<Precondition>(yaml).unwrap();

      assert_eq!(precondition.command, "echo \"Hello, World!\"");
      assert_eq!(precondition.message, None);
    }
  }
}
