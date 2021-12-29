// Port of https://github.com/tannerlinsley/react-query/blob/master/examples/basic

use reqwasm::http::Request;
use serde::Deserialize;
use yew::{function_component, html, Callback, Html, Properties};
use yew_query::{query_response, use_query, QueryState, QueryStatus as Status, QueryClientProvider, QueryClient};

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct Post {
    id: usize,
    title: String,
    body: String,
}

query_response! {
    Response {
        posts -> Vec<Post>,
        post -> Post,
    }
}

fn use_posts() -> QueryState<Response> {
    use_query(
        "posts",
        |_| {
            Box::pin(async {
                Ok(Response::Posts(
                    Request::get("https://jsonplaceholder.typicode.com/posts")
                        .send()
                        .await
                        .unwrap()
                        .json()
                        .await
                        .unwrap(),
                ))
            })
        },
        None,
    )
}

#[derive(Clone, Properties, PartialEq)]
struct PostProps {
    set_post_id: Callback<usize>,
}

#[function_component(Posts)]
fn posts(props: &PostProps) -> Html {
    let posts = use_posts();

    html! {
        <div>
            <h1>{ "Posts" }</h1>
            <div>
                {
                    match posts.status {
                        Status::Loading => html! { "Loading..." },
                        Status::Success(data) => {
                            let posts_iter = data
                                    .get_posts()
                                    .iter()
                                    .map(|post| {
                                        let set_post_id = props.set_post_id.clone();
                                        html! {
                                            <p>
                                                <a onclick={ move |_| set_post_id.emit(post.id) } href="#">
                                                    { post.title.clone() }
                                                </a>
                                            </p>
                                        }
                                    });

                            html! {
                                <>
                                    <div>
                                        { for posts_iter }
                                    </div>
                                    <div>{ if post.is_fetching { html! { "Background Updating..." } } }</div>
                                </>
                            }
                        },
                        Status::Error(_error) => html! {
                            <span>{ "Error" }</span>
                        }
                    }
                }
            </div>
        </div>
    }
}

fn get_post_by_id(id: usize) {
    Request::get(format!("https://jsonplaceholder.typicode.com/posts/{}", id))
        .send()
        .await
        .unwrap()
        .json::<Post>()
        .await
        .unwrap()
}

fn use_post(post_id: usize) {
    use_query(
        format!("post/{}", post_id).as_ref(),
        |_| Box::pin(async move {
          Ok(Response::Post(get_post_id(post_id).await))
        }),
        None
    )
}

#[derive(Clone, Properties, PartialEq)]
struct PostProps {
    post_id: usize,
    set_post_id: Callback<usize>,
}

#[function_component(Post)]
fn post(props: &PostProps) -> Html {
    let post = use_post(props.post_id);
    let set_post_id = props.set_post_id.clone();

    html! {
        <div>
            <div>
                <a onclick={ move |_| set_post_id.emit(props.post_id)  } href="#">
                    { "Back" }
                </a>
            </div>
            {
                match post.status {
                    Status::Loading => html! { "Loading..." },
                    Status::Success(data) => {
                        let post_data = data.get_post();
                        html! {
                            <>
                                <h1>{ post_data.title.clone() }</h1>
                                <div>
                                    <p>{ post_data.body.clone() }</p>
                                </div>
                                <div>{ if post.is_fetching { html! { "Background Updating..." } } }</div>
                            </>
                        }
                    },
                    Status::Error(_error) => html! {
                        <span>{ "Error" }</span>
                    }
                }
            }
        </div>
    }
}

#[function_component(App)]
fn app() -> Html {
    let client = use_state(|| QueryClient::<Response>::new())
    let post_id = use_state(|| Option::<usize>::None);

    let set_post_id = Callback::from(move |id| post_id.set(id));

    html! {
        <QueryClientProvider<Response> client={(*client).clone()}>
            <p>
                { "As you visit the posts below, you will notice them in a loading state the first time you load them. However, after you return to this list and click on any posts you have already visited again, you will see them load instantly and background refresh right before your eyes!" }
                <strong>
                  { "(You may need to throttle your network speed to simulate longer loading sequences)" }
                </strong>
            </p>
            {
                if post_id.is_none() {
                    html! { <Posts set_post_id={set_post_id.clone()} /> }
                } else {
                    html! { <Post post_id={post_id.clone().unwrap()} set_post_id={set_post_id.clone()} /> }
                }
            }
        </QueryClientProvider<Response>>
    }
}

fn main() {
    yew::start_app::<App>();
}
