#![crate_type = "rlib"]

//! Macro to create a "view box", a box containing immutable data plus
//! a "view" struct that can have interior references into the data.
//! The view box can be moved around as an atomic unit.  I haven't
//! convinced myself this is memory safe yet.  Known issues:
//!
//! * If the view type implements Drop (via #[unsafe_destructor]), it
//! could potentially observe dangling references depending on the
//! order in which struct fields are destroyed.  Currently, struct
//! fields seem to be destroyed in reverse order, which means things
//! should work fine as-is, but this depends on implementation details
//! of the compiler.  This could be fixed trivially by changing the
//! struct internals and adding a Drop impl that forces the view
//! to be destroyed first (e.g. by putting it in an Option and calling
//! .take()) at the cost of memory usage.
#![feature(macro_rules, globs)]

#[macro_export]
macro_rules! viewbox(
    (struct $name:ident<$d:ty, $v:ident>;) => (
        pub struct $name {
            data: Box<$d>,
            view: $v<'static>
        }

        #[allow(dead_code)]
        impl $name {
            pub fn new(data: $d, f: <'a>|&'a mut $d| -> $v<'a>) -> $name {
                let mut d = box data;
                let v = unsafe { ::std::mem::transmute(f(&mut *d)) };
                
                $name { data: d, view: v }
            }

            pub fn new_result<E>(data: $d,
                                 f: <'a>|&'a mut $d| 
                                    -> ::std::result::Result<$v<'a>,E>)
                                 -> ::std::result::Result<$name,($d,E)> {
                let mut d = box data;
                match f(&mut *d).map(|v| unsafe { ::std::mem::transmute(v) }) {
                    Ok(v) => Ok($name { data: d, view: v }),
                    Err(e) => Err((*d,e))
                }
            }
            
            pub fn unwrap(self)-> $d {
                let $name { data: box data, .. } = self;
                data
            }

            pub fn view<'a>(&'a self) -> &'a $v<'a> {
                unsafe { ::std::mem::transmute(&self.view) }
            }

            pub fn mut_view<'a>(&'a mut self) -> &'a mut $v<'a> {
                unsafe { ::std::mem::transmute(&mut self.view) }
            }
        }
    );
    (#[deriving(PartialEq $(,$derive:ident)*)] struct $name:ident<$d:ty, $v:ident>;) => (
        impl ::std::cmp::PartialEq for $name {
            fn eq(&self, other: &$name) -> bool { self.view() == other.view(); }
        }
        viewbox!(struct $name<$d,$v>;)
    );
    (#[deriving(Show $(,$derive:ident)*)] struct $name:ident<$d:ty, $v:ident>;) => (
        impl ::std::fmt::Show for $name {
            fn fmt(&self, fmt: &mut ::std::fmt::Formatter)
                   -> ::std::result::Result<(), ::std::fmt::FormatError> {
                self.view().fmt(fmt)
            }
        }
        viewbox!(struct $name<$d,$v>;)
    );
    (#[deriving()] struct $name:ident<$d:ty, $v:ident>;) => (
        viewbox!(struct $name<$d,$v>;)
    );
)

#[cfg(test)]
mod test {
    // Test data structure
    #[deriving(PartialEq,Show)]
    struct TestData {
        foo: i32,
        bar: String
    }
    
    // View structure that has references into the boxed data
    struct TestView<'a> {
        x: &'a i32,
        y: &'a str
    }
    
    // Create TestBox which combines TestData and TestView
    viewbox! {
        struct TestBox<TestData,TestView>;
    }
    
    // Mutable view into TestData
    struct MutView<'a> {
        x: &'a mut i32,
        y: &'a mut String
    }

    // MutBox combines TestData and MutView
    viewbox! {
        struct MutBox<TestData, MutView>;
    }

    #[test]
    fn basic() {
        // Create some data
        let t = TestData { foo: 42, bar: "Hello".to_string() };
        // Move it into box, creating a view with interior references
        let v = TestBox::new(t, |d| TestView { x: &d.foo, y: d.bar.as_slice() });
        
        // We can access the boxed data via the view
        assert_eq!(v.view().x, &42i32);
        assert_eq!(v.view().y, "Hello");
        // We can unwrap the view to get the data back
        let t = v.unwrap();
        assert_eq!(t.foo, 42i32);
        assert_eq!(t.bar, "Hello".to_string());
    }

    #[test]
    fn mutation() {
        let t = TestData { foo: 42, bar: "Hello".to_string() };
        let mut v = MutBox::new(t, |d| MutView { x: &mut d.foo, y: &mut d.bar });
        
        *v.mut_view().x = 5;
        *v.mut_view().y = "Goodbye".to_string();
        let t = v.unwrap();
        assert_eq!(t, TestData { foo: 5, bar: "Goodbye".to_string() });
    }

    #[test]
    fn error() {
        let t = TestData { foo: 42, bar: "Hello".to_string() };
        let v = MutBox::new_result(t, |_| Err("Nope")).err().unwrap();
        assert_eq!(v, (TestData { foo: 42, bar: "Hello".to_string() }, "Nope"))
    }
}
