use std::cmp::PartialEq;
use std::fmt::{self, Debug};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

pub struct FnPtr<Arg, Rt> {
    cb: Rc<dyn Fn(Arg) -> Pin<Box<dyn Future<Output = Rt>>>>,
}

impl<Arg, Rt, F> From<F> for FnPtr<Arg, Rt>
where
    F: 'static + Fn(Arg) -> Pin<Box<dyn Future<Output = Rt>>>,
{
    fn from(func: F) -> Self {
        FnPtr { cb: Rc::new(func) }
    }
}

impl<Arg, Rt> PartialEq for FnPtr<Arg, Rt> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.cb, &other.cb)
    }
}

impl<Arg, Rt> Clone for FnPtr<Arg, Rt> {
    fn clone(&self) -> Self {
        Self {
            cb: self.cb.clone(),
        }
    }
}

impl<Arg, Rt> fmt::Debug for FnPtr<Arg, Rt> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FnPtr<_>")
    }
}

impl<Arg, Rt> FnPtr<Arg, Rt> {
    pub async fn emit(&self, value: Arg) -> Rt {
        let x = (self.cb)(value);
        x.await
    }
}

pub fn now() -> i64 {
    instant::now() as i64
}

pub type QueryResult<TData> = Result<TData, String>;

mod utils {
    use super::{now, FnPtr, QueryResult};
    use std::cell::RefCell;
    use std::cmp::PartialEq;
    use std::fmt::Debug;
    use std::rc::Rc;
    use yew::Callback;

    #[derive(Clone)]
    pub struct QueryOptions<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        pub query_key: String,
        pub query_fn: FnPtr<(), QueryResult<TData>>,
        pub stale_time: i64,
    }

    #[derive(PartialEq, Debug)]
    pub struct QueryClient<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        pub queries: Rc<RefCell<Vec<Rc<RefCell<Query<TData>>>>>>,
    }

    impl<TData> Clone for QueryClient<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        fn clone(&self) -> Self {
            // web_sys::console::log_1(&format!("CLONING QUERY CLIENT {:#?}", self).into());
            Self {
                queries: Rc::clone(&self.queries),
            }
        }
    }

    impl<TData> QueryClient<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        pub fn new() -> Self {
            Self {
                queries: Rc::new(RefCell::new(vec![])),
            }
        }

        fn get_query(&mut self, options: &QueryOptions<TData>) -> Rc<RefCell<Query<TData>>> {
            let query_key = options.query_key.clone();
            let mut queries = (*self.queries).borrow_mut();
            let query = queries
                .iter()
                .find(|&query| query.borrow().query_key == query_key);

            // web_sys::console::log_1(&format!("{:#?}", self).into());

            if let Some(query) = query {
                web_sys::console::log_1(&format!("query found {:#?}", *query).into());
                Rc::clone(query)
            } else {
                let query = Rc::new(RefCell::new(create_query(&options)));
                queries.push(Rc::clone(&query));
                // web_sys::console::log_1(&format!("Updated: {:#?}", self).into());

                query
            }
        }
    }

    #[derive(Clone, PartialEq, Debug)]
    pub enum QueryStatus<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        Loading,
        Success(TData),
        Error(String),
    }

    #[derive(Clone, PartialEq, Debug)]
    pub struct Query<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        pub state: QueryState<TData>,
        pub query_fn: FnPtr<(), QueryResult<TData>>,
        pub subscribers: Vec<Callback<()>>,
        pub query_key: String,
    }

    impl<TData> Query<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        pub async fn fetch(&mut self) {
            web_sys::console::log_1(&"updating state".into());
            web_sys::console::log_1(&format!("{:#?}", self.state).into());

            self.set_state(|old| QueryState {
                is_fetching: true,
                ..old
            });

            match self.query_fn.emit(()).await {
                Ok(data) => {
                    self.set_state(|old| QueryState {
                        status: QueryStatus::Success(data.clone()),
                        last_updated: Some(now()),
                        ..old
                    });
                }
                Err(err) => self.set_state(|old| QueryState {
                    status: QueryStatus::Error(err.clone()),
                    ..old
                }),
            };

            self.set_state(|old| QueryState {
                is_fetching: false,
                ..old
            });

            web_sys::console::log_1(&"new state".into());
            web_sys::console::log_1(&format!("{:#?}", self.state).into());
        }

        fn set_state(&mut self, updater: impl Fn(QueryState<TData>) -> QueryState<TData>) {
            self.state = updater(self.state.clone());
            for cb in &self.subscribers {
                cb.emit(());
            }
        }

        fn subscribe(&mut self, _subscriber: &Subscriber<TData>, callback: Callback<()>) {
            self.subscribers.push(callback);
        }

        // fn unsubscribe(&mut self, subscriber: &Subscriber<T, TData>) {
        //     self.subscribers = self
        //         .subscribers
        //         .clone()
        //         .into_iter()
        //         .filter(|sub| sub == subscriber)
        //         .collect::<Vec<_>>();
        // }
    }

    #[derive(Clone, PartialEq, Debug)]
    pub struct QueryState<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        pub status: QueryStatus<TData>,
        pub is_fetching: bool,
        pub last_updated: Option<i64>,
    }

    impl<TData> QueryState<TData> where TData: Clone + PartialEq + Debug {}

    fn create_query<TData>(options: &QueryOptions<TData>) -> Query<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        let query = Query {
            state: QueryState {
                status: QueryStatus::Loading,
                is_fetching: true,
                last_updated: None,
            },
            query_fn: options.query_fn.clone(),
            subscribers: vec![],
            query_key: options.query_key.clone(),
        };

        query
    }

    #[derive(Clone, PartialEq, Debug)]
    pub struct Subscriber<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        query: Rc<RefCell<Query<TData>>>,
        stale_time: i64,
    }

    impl<TData> Subscriber<TData>
    where
        TData: Clone + PartialEq + Debug + 'static,
    {
        pub fn get_result(&self) -> QueryState<TData> {
            let y = Rc::clone(&self.query);
            web_sys::console::log_1(&"`get_result`: TRYING TO BORROW".into());
            let x = (*y).borrow();
            let state = x.state.clone();
            std::mem::drop(x);
            state
        }

        pub fn subscribe(&mut self, callback: Callback<()>) {
            web_sys::console::log_1(&"`subscribe`: TRYING TO BORROW".into());
            let mut x = (*self.query).borrow_mut();
            x.subscribe(&self, callback);
            std::mem::drop(x);
            self.fetch();
        }

        pub fn fetch(&mut self) {
            web_sys::console::log_1(&"`fetch`: TRYING TO BORROW MUT".into());
            let query = Rc::clone(&self.query);
            let query = (*query).borrow_mut();
            if query.state.last_updated.is_none()
                || ((now()) - query.state.last_updated.unwrap() > self.stale_time)
            {
                let mut query = query.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    web_sys::console::log_1(&"`spawn_local`: TRYING TO BORROWING MUT".into());
                    // >> ISSUE OCCURS HERE
                    query.fetch().await;
                    web_sys::console::log_1(&"`spawn_local`: TRYING TO DROP MUT".into());
                });
            }
        }
    }

    pub fn create_query_observer<TData>(
        client: &mut QueryClient<TData>,
        options: QueryOptions<TData>,
    ) -> Subscriber<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        let query = client.get_query(&options);
        // web_sys::console::log_1(&format!("{:#?}", query).into());
        let observer = Subscriber {
            query,
            stale_time: options.stale_time,
        };

        observer
    }
}

pub use utils::{Query, QueryClient, QueryOptions, QueryState, QueryStatus};
use yew::{
    function_component, html, use_context, use_effect_with_deps, use_mut_ref, use_state, Callback,
    Children, ContextProvider, Properties,
};

pub fn use_query<TData, F>(
    query_key: &str,
    query_fn: F,
    stale_time: Option<i64>,
) -> utils::QueryState<TData>
where
    TData: Clone + PartialEq + Debug + 'static,
    F: 'static + Fn(()) -> Pin<Box<dyn Future<Output = Result<TData, String>>>>,
{
    let query_fn = FnPtr::from(query_fn);
    let mut client = use_query_client::<TData>();
    // web_sys::console::log_1(&format!("{:#?}", client).into());
    let rerender = {
        let c = use_state(|| 0);
        move || {
            c.set(*c + 1);
        }
    };
    let observer_ref = use_mut_ref(|| {
        web_sys::console::log_1(&"created query observer".into());

        utils::create_query_observer(
            &mut client,
            utils::QueryOptions {
                query_fn,
                query_key: String::from(query_key),
                stale_time: stale_time.unwrap_or(0),
            },
        )
    });

    {
        let observer_ref = observer_ref.clone();
        let rerender = rerender.clone();

        use_effect_with_deps(
            move |_| {
                web_sys::console::log_1(&"rerender".into());
                observer_ref
                    .borrow_mut()
                    .subscribe(Callback::from(move |_| rerender()));

                || ()
            },
            (),
        );
    }

    let result = observer_ref.borrow_mut().get_result();
    result
}

#[derive(Properties, PartialEq)]
pub struct QueryClientProviderProps<T>
where
    T: Clone + Debug + PartialEq,
{
    pub client: QueryClient<T>,
    #[prop_or_default]
    pub children: Children,
}

pub fn use_query_client<TData>() -> QueryClient<TData>
where
    TData: Clone + PartialEq + Debug + 'static,
{
    use_context::<QueryClient<TData>>().expect("QueryContext not found")
}

#[function_component(QueryClientProvider)]
pub fn query_client_provider<T>(props: &QueryClientProviderProps<T>) -> Html
where
    T: Clone + Debug + PartialEq + 'static,
{
    html! {
        <ContextProvider<QueryClient<T>> context={props.client.clone()}>
            { for props.children.iter() }
        </ContextProvider<QueryClient<T>>>
    }
}

pub use paste;

// TODO convert to `derive` macro
#[macro_export]
macro_rules! query_response {
    ($enum_name:ident {
        $( $field:ident -> $type:ty ),*
    }) => {
        use yew_query::paste::paste;

        paste! {
            #[derive(Clone, PartialEq, Debug)]
            pub enum $enum_name {
                $(
                    [<$field:camel>]($type),
                )*
            }

            impl $enum_name {
                $(
                  pub fn [<get_ $field:lower>](&self) -> &$type {
                      match &self {
                          &$enum_name::[<$field:camel>](ref x) => x,
                          &x => panic!("Expected: {}, Found: {}", format!("{}::{}{}", stringify!($enum_name), stringify!([<$field:camel>]), stringify!($type)), format!("{}::{:?}", stringify!($enum_name), x))
                      }
                  }
                )*
            }
        }
    }
}
