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
    use wasm_bindgen::JsCast;
    use yew::Callback;

    #[derive(Clone)]
    pub struct QueryOptions<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        pub query_key: String,
        pub query_fn: FnPtr<(), QueryResult<TData>>,
        pub stale_time: i64,
        pub cache_time: i32,
    }

    #[derive(PartialEq, Debug)]
    pub struct QueryClient<TData>
    where
        TData: Clone + PartialEq + Debug + 'static,
    {
        pub queries: Rc<RefCell<Vec<Rc<RefCell<Query<TData>>>>>>,
        subscribers: Rc<RefCell<Vec<Callback<()>>>>,
    }

    impl<TData> Clone for QueryClient<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        fn clone(&self) -> Self {
            // web_sys::console::log_1(&format!("CLONING QUERY CLIENT {:#?}", self).into());
            Self {
                queries: Rc::clone(&self.queries),
                subscribers: Rc::clone(&self.subscribers),
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
                subscribers: Rc::new(RefCell::new(vec![])),
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
                let query = Rc::new(RefCell::new(create_query(self.clone(), &options)));
                queries.push(Rc::clone(&query));
                // web_sys::console::log_1(&format!("Updated: {:#?}", self).into());

                query
            }
        }

        pub fn subscribe(&mut self, callback: Callback<()>) {
            (*self.subscribers).borrow_mut().push(callback);
        }

        pub fn unsubscribe(&mut self, callback: Callback<()>) {
            (*self.subscribers)
                .borrow_mut()
                .retain(|subscriber| subscriber.clone() == callback)
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
        TData: Clone + PartialEq + Debug + 'static,
    {
        // change to lifetime reference
        client: QueryClient<TData>,
        pub state: QueryState<TData>,
        pub query_fn: FnPtr<(), QueryResult<TData>>,
        pub subscribers: Vec<(Subscriber<TData>, Callback<()>)>,
        pub query_key: String,
        pub cache_time: i32,
        timeout: Option<i32>,
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
            for (_, cb) in &self.subscribers {
                cb.emit(());
            }
        }

        fn subscribe(&mut self, subscriber: Subscriber<TData>, callback: Callback<()>) {
            self.subscribers.push((subscriber, callback));
            self.unschedule_query_cleanup();
        }

        fn unsubscribe(&mut self, callback: Callback<()>) {
            self.subscribers = self
                .subscribers
                .iter()
                .cloned()
                // if stored callback and callback passed to `unsubscribe`
                // are equal, then the subscribers should also be equal
                // since they are created at the same time
                .filter(|(_, cb)| cb.clone() == callback)
                .collect::<Vec<_>>();

            if self.subscribers.len() == 0 {
                self.schedule_query_cleanup();
            }
        }

        fn schedule_query_cleanup(&mut self) {
            let timeout = web_sys::window()
                .expect("Couldn't access `window`")
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    wasm_bindgen::closure::Closure::wrap(Box::new(|| {
                        web_sys::console::log_1(&"works i guess".into())
                    })
                        as Box<dyn FnMut()>)
                    .as_ref()
                    .unchecked_ref(),
                    self.cache_time,
                )
                .expect("`setTimeout` didn't register");

            self.timeout = Some(timeout);
        }

        fn unschedule_query_cleanup(&mut self) {
            if let Some(timeout) = self.timeout {
                web_sys::window()
                    .expect("Couldn't access `window`")
                    .clear_timeout_with_handle(timeout)
            }
        }
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

    fn create_query<TData>(
        client: QueryClient<TData>,
        options: &QueryOptions<TData>,
    ) -> Query<TData>
    where
        TData: Clone + PartialEq + Debug,
    {
        Query {
            client,
            state: QueryState {
                status: QueryStatus::Loading,
                is_fetching: true,
                last_updated: None,
            },
            query_fn: options.query_fn.clone(),
            subscribers: vec![],
            query_key: options.query_key.clone(),
            cache_time: options.cache_time,
            timeout: None,
        }
    }

    #[derive(Clone, PartialEq, Debug)]
    pub struct Subscriber<TData>
    where
        TData: Clone + PartialEq + Debug + 'static,
    {
        query: Rc<RefCell<Query<TData>>>,
        stale_time: i64,
        cache_time: i32,
    }

    impl<T> Drop for Subscriber<T>
    where
        T: Clone + PartialEq + Debug + 'static,
    {
        fn drop(&mut self) {
            web_sys::console::log_1(&"DROPPING SUBSCRIBER".into())
        }
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
            x.subscribe(self.clone(), callback);
            std::mem::drop(x);
            self.fetch();
        }

        pub fn unsubscribe(&mut self, callback: Callback<()>) {
            (*self.query).borrow_mut().unsubscribe(callback)
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
        Subscriber {
            query,
            stale_time: options.stale_time,
            cache_time: options.cache_time,
        }
    }
}

pub use utils::{Query, QueryClient, QueryOptions, QueryState, QueryStatus};
use wasm_bindgen::JsCast;
use web_sys::window;
use yew::{
    function_component, html, use_context, use_effect_with_deps, use_mut_ref, use_state, Callback,
    Children, ContextProvider, Properties,
};

pub struct UseQueryOptions {
    pub stale_time: Option<i64>,
    pub cache_time: Option<i32>,
}

impl Default for UseQueryOptions {
    fn default() -> Self {
        Self {
            stale_time: None,
            // `cache_time` might not be required in rust, since its only there to trigger
            // GC in react-query, which shouldn't be required in rust, since it doesn't have GC
            // but Query is being stored in an `Rc`, so it wouldn't be dropped from memory
            // automatically, unless all references are removed
            // for which it would need to be removed from the `queries` vec.
            cache_time: None,
        }
    }
}

const FIX_MINUTES_MS: i32 = 5 * 60 * 1000;

pub fn use_query<TData, F>(
    query_key: &str,
    query_fn: F,
    options: UseQueryOptions,
) -> utils::QueryState<TData>
where
    TData: Clone + PartialEq + Debug + 'static,
    F: 'static + Fn(()) -> Pin<Box<dyn Future<Output = Result<TData, String>>>>,
{
    let query_fn = FnPtr::from(query_fn);
    let mut client = use_query_client::<TData>();

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
                stale_time: options.stale_time.unwrap_or(0),
                cache_time: options.cache_time.unwrap_or(FIX_MINUTES_MS),
            },
        )
    });

    {
        let observer_ref = observer_ref.clone();
        let rerender = rerender.clone();

        use_effect_with_deps(
            move |_| {
                web_sys::console::log_1(&"rerender".into());

                let cb = Callback::<()>::from(move |_| rerender());
                let mut observer = observer_ref.borrow_mut();
                observer.subscribe(cb.clone());

                {
                    let mut observer = observer.clone();
                    move || observer.unsubscribe(cb.clone())
                }
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
    T: Clone + Debug + PartialEq + 'static,
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
    let client = props.client.clone();

    {
        let queries = client.queries.clone();

        use_effect_with_deps(
            move |_| {
                let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                    for query in (queries).borrow_mut().iter() {
                        for (subscriber, _) in &mut query.borrow_mut().subscribers {
                            subscriber.fetch()
                        }
                    }
                })
                    as Box<dyn FnMut()>);
                let on_focus = closure.as_ref().unchecked_ref::<js_sys::Function>();

                let window = window().expect("Couldn't access `window`");

                window
                    .add_event_listener_with_callback_and_bool(
                        "visibilitychange",
                        &on_focus.clone(),
                        false,
                    )
                    .unwrap();
                window
                    .add_event_listener_with_callback_and_bool("focus", &on_focus.clone(), false)
                    .unwrap();

                {
                    let on_focus = on_focus.clone();

                    move || {
                        window
                            .remove_event_listener_with_callback(
                                "visibilitychange",
                                &on_focus.clone(),
                            )
                            .unwrap();
                        window
                            .remove_event_listener_with_callback("focus", &on_focus.clone())
                            .unwrap()
                    }
                }
            },
            client,
        );
    }

    html! {
        <ContextProvider<QueryClient<T>> context={props.client.clone()}>
            { for props.children.iter() }
        </ContextProvider<QueryClient<T>>>
    }
}

// #[cfg(feature = "devtools")]
pub mod devtools {
    use crate::{use_query_client, utils::QueryStatus};
    use yew::{function_component, html, use_effect_with_deps, use_state, Callback};

    #[function_component(QueryDevtools)]
    pub fn query_devtools<TData>() -> Html
    where
        TData: Clone + PartialEq + std::fmt::Debug + 'static,
    {
        let mut client = use_query_client::<TData>();
        let rerender = {
            let c = use_state(|| 0);
            Callback::from(move |_: ()| {
                c.set(*c + 1);
            })
        };
        let mut queries = {
            let queries = (*client.queries).clone();

            queries
                .borrow_mut()
                .sort_by_cached_key(|query| (*query).borrow().query_key.clone());

            queries
        };
        let queries = queries.get_mut().iter().map(|query| {
            let query = (**query).borrow();

            html! {
                <div style="">
                    { format!("\"{}\" -", query.query_key.clone()) }
                    <span style="">
                        { if query.state.is_fetching {
                            html! { <span style="">{ "fetching" }</span> }
                        } else if query.subscribers.len() == 0 {
                            html! { <span style="">{ "inactive" }</span> }
                        } else if let QueryStatus::Success(_) = query.state.status {
                            html! { <span style="">{ "success" }</span> }
                        } else if let QueryStatus::Error(_) = query.state.status {
                            html! { <span style="">{ "error" }</span> }
                        } else {
                            html! {}
                        } }
                    </span>
                </div>
            }
        });

        {
            let mut client = client.clone();

            use_effect_with_deps(
                move |_| {
                    client.subscribe(rerender.clone());

                    move || client.unsubscribe(rerender.clone())
                },
                (),
            )
        }

        html! {
            <div style="background-color: black; color: white;">
                {for queries}
            </div>
        }
    }
}

pub mod __private {
    pub use paste;
}

#[macro_export]
macro_rules! query_response {
    ($enum_name:ident {
        $( $field:ident -> $type:ty ),*
    }) => {
        yew_query::__private::paste::paste! {
            #[derive(Clone, PartialEq, Debug)]
            pub enum $enum_name {
                $(
                    [<$field:camel>]($type),
                )*
            }

            fn panic_unexpected_type(field: &str, ty: &str, x: &$enum_name) -> ! {
              let expected = format!("{}::{}({})", stringify!($enum_name), field, ty);
              let found = format!("{}::{:?}", stringify!($enum_name), x);

              panic!("Expected: {}, Found: {}", expected, found)
            }

            impl $enum_name {
                $(
                  pub fn [<get_ $field:lower>](&self) -> &$type {
                      match &self {
                            &$enum_name::[<$field:camel>](ref x) => x,
                            &unknown => panic_unexpected_type(stringify!([<$field:camel>]), stringify!($type), &unknown)
                      }
                  }
                )*
            }

            // $(
            //     impl From<$type> for $enum_name {
            //         fn from([<$field:lower>]: $type) -> Self {
            //             Self::[<$field:camel>]([<$field:lower>])
            //         }
            //     }
            // )*
        }
    }
}
