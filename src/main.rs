use std::borrow::Cow ;
use std::rc::Rc ;
use std::sync::Arc ;

fn log_inner(s: &str) {
    println!("{}", s)
}

fn log_outer<T: AsRef<str>>(s: T) {
    log_inner(s.as_ref());
}

struct MyString(String) ;

impl AsRef<str> for MyString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

fn main() {
    // 1. &str
    log_outer("&str");

    // 2. String
    log_outer(String::from("String"));

    // 3. &String
    let s = String::from("&String");
    log_outer(&s);

    // 4. Box<str>
    let s = Box::<str>::from("Box<str>") ;
    log_outer(s);

    // 5. Cow<'_, str>
    let s = Cow::Borrowed("Cow<'_, str>") ;
    log_outer(s);

    // 6. Cow<'_, String>
    let s = Cow::<String>::Owned("Cow<'_, String>".into()) ;
    log_outer(s.as_ref());

    // 7. Rc<str>
    let s = Rc::<str>::from("Rc<str>") ;
    log_outer(s);

    // 8. Arc<str>
    let s = Arc::<str>::from("Arc<str>") ;
    log_outer(s);

    // 9. MyString
    let s = MyString("MyString".into()) ;
    log_outer(s.as_ref());

} /* Out:
&str
String
&String
Box<str>
Cow<'_, str>
Cow<'_, String>
Rc<str>
Arc<str>
MyString
*/
