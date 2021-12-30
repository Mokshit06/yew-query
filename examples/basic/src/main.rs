// Port of https://github.com/tannerlinsley/react-query/blob/master/examples/basic

use reqwasm::http::Request;
use serde::Deserialize;
use yew::{function_component, html, use_state, Callback, Html, Properties};
use yew_query::devtools::QueryDevtools;
use yew_query::{
    query_response, use_query, QueryClient, QueryClientProvider, QueryOptions, QueryResult,
    QueryState, Status,
};

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct Post {
    id: usize,
    title: String,
    body: String,
}

query_response! {
    Response {
        post -> Post,
        posts -> Vec<Post>
    }
}

async fn get_posts() -> QueryResult<Response> {
    Ok(Response::Posts(
        Request::get("https://jsonplaceholder.typicode.com/posts")
            .send()
            .await
            .map_err(|err| err.to_string())?
            .json()
            .await
            .map_err(|err| err.to_string())?,
    ))
}

fn use_posts() -> QueryState<Response> {
    use_query(
        "posts",
        |_| Box::pin(get_posts()),
        QueryOptions {
            stale_time: Some(3000),
            ..Default::default()
        },
    )
}

#[derive(Clone, Properties, PartialEq)]
struct PostsProps {
    set_post_id: Callback<usize>,
}

#[function_component(Posts)]
fn posts(props: &PostsProps) -> Html {
    let posts = use_posts();
    let set_post_id = props.set_post_id.clone();

    html! {
        <div>
            <h1>{ "Posts" }</h1>
            <div>
                {
                    match posts.status {
                        Status::Idle => html! {},
                        Status::Loading => html! { "Loading..." },
                        Status::Success(data) => {
                            html! {
                                <>
                                    <div>
                                        { data
                                            .get_posts()
                                            .to_owned()
                                            .iter()
                                            .map(|post| {
                                                let post = post.clone();
                                                let set_post_id = set_post_id.clone();

                                                html! {
                                                    <a
                                                        onclick={move |_| set_post_id.emit(post.id.clone()) }
                                                        href="#"
                                                    >
                                                        { post.title.clone() }
                                                    </a>
                                                }
                                            })
                                            .collect::<Html>()
                                        }
                                    </div>
                                    <div>{
                                      if posts.is_fetching {
                                        html! { "Background Updating..." }
                                      } else {
                                        html! {}
                                      }
                                    }</div>
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

async fn get_post_by_id(id: usize) -> QueryResult<Response> {
    Ok(Response::Post(
        Request::get(format!("https://jsonplaceholder.typicode.com/posts/{}", id).as_ref())
            .send()
            .await
            .map_err(|err| err.to_string())?
            .json::<Post>()
            .await
            .map_err(|err| err.to_string())?,
    ))
}

fn use_post(post_id: usize) -> QueryState<Response> {
    use_query(
        format!("post/{}", post_id).as_ref(),
        move |_| Box::pin(get_post_by_id(post_id)),
        QueryOptions::default(),
    )
}

#[derive(Clone, Properties, PartialEq)]
struct SinglePostProps {
    post_id: usize,
    set_post_id: Callback<usize>,
}

#[function_component(SinglePost)]
fn post(props: &SinglePostProps) -> Html {
    let post = use_post(props.post_id);
    let post_id = props.post_id.clone();
    let set_post_id = props.set_post_id.clone();

    html! {
        <div>
            <div>
                <a onclick={ move |_| set_post_id.emit(post_id)  } href="#">
                    { "Back" }
                </a>
            </div>
            {
                match post.status {
                    Status::Idle => html! {},
                    Status::Loading => html! { "Loading..." },
                    Status::Success(data) => {
                        let post_data = data.get_post();
                        html! {
                            <>
                                <h1>{ post_data.title.clone() }</h1>
                                <div>
                                    <p>{ post_data.body.clone() }</p>
                                </div>
                                <div>{
                                    if post.is_fetching {
                                        html! { "Background Updating..." }
                                    } else {
                                        html! {}
                                    }
                                }</div>
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
    let client = use_state(|| QueryClient::<Response>::new());
    let post_id = use_state(|| Option::<usize>::None);

    let set_post_id = {
        let post_id = post_id.clone();
        Callback::from(move |id| post_id.set(Some(id)))
    };

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
                    html! { <Posts set_post_id={set_post_id} /> }
                } else {
                    html! { <SinglePost post_id={post_id.clone().unwrap()} set_post_id={set_post_id} /> }
                }
            }
            <QueryDevtools<Response> />
        </QueryClientProvider<Response>>
    }
}

fn main() {
    yew::start_app::<App>();
}
