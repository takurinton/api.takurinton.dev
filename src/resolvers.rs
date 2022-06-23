use chrono::{DateTime, Utc};
use sqlx::{mysql::MySqlPool};
use std::{env};

use async_graphql::{
  Object,
  Context,
  SimpleObject,
  ErrorExtensions, 
  FieldError, 
};

#[derive(SimpleObject)]
#[derive(sqlx::FromRow)]
struct Ping {
  status: String,
  code: i32,
}

#[derive(sqlx::FromRow)]
struct Count {
  count: i64,
}

#[derive(SimpleObject)]
#[derive(sqlx::FromRow)]
pub struct Post {
  id: i32,
  title: String,
  category: Option<String>,
  contents: Option<String>,
  pub_date: DateTime<Utc>,
  open: i8,
}


#[derive(SimpleObject)]
pub struct Posts {
  current: i32,
  next: Option<i32>,
  prev: Option<i32>,
  category: String,
  page_size: i32,
  results: Vec<Post>,
}

pub struct QueryRoot;

#[derive(Debug, Error)]
pub enum BlogError {
    #[error("Could not find resource")]
    NotFound,

    #[error("ServerError")]
    ServerError(String),

    #[error("No Extensions")]
    ErrorWithoutExtensions,
}

impl ErrorExtensions for BlogError {
  fn extend(&self) -> FieldError {
      self.extend_with(|err, e| match err {
        BlogError::NotFound => e.set("code", "NOT_FOUND"),
        BlogError::ServerError(reason) => e.set("reason", reason.to_string()),
        BlogError::ErrorWithoutExtensions => {}
      })
  }
}

/**
 * resolvers
 */
#[Object]
impl QueryRoot {
  async fn ping(&self) -> Ping {
    Ping { 
      status: "ok".to_string(), 
      code: 200 
    }
  }

  #[allow(non_snake_case)]
  async fn getPost(
      &self,
      _ctx: &Context<'_>,
      #[graphql(desc = "id of the post")] id: i32,
  ) -> Post {
    let post = get_post(id).await.unwrap();
    post
  }

  #[allow(non_snake_case)]
  async fn getPosts(
      &self, 
      _ctx: &Context<'_>,
      #[graphql(desc = "current page")] page: i32, 
      #[graphql(desc = "selected category")] category: String
  ) -> Posts {
    let convertPage = if page == 0 { 1 } else { page };
    let categoryForResult = category.clone();
    let results = get_posts(page, category).await.unwrap();
    let count = count().await.unwrap();
    let page_size = (count / 5) + 1;
      Posts {
          current: convertPage,
          next: if convertPage == page_size { Some(page_size) } else { Some(convertPage + 1) },
          prev: if convertPage == 0 { Some(0) } else { Some(convertPage - 1) },
          category: categoryForResult,
          page_size,
          results,
      }
  }
}

/**
 * database
 */

// count all posts
pub async fn count() -> Option<i32> {
  let uri = &env::var("DATABASE_URL").unwrap();
  let pool = MySqlPool::connect(uri).await.unwrap();
  let count_all = sqlx::query_as::<_, Count>(
    r#"
SELECT count(*) as count FROM blogapp_post where open = true
    "#
)
  .fetch_one(&pool)
  .await;

  match count_all {
    Ok(count) => Some(count.count as i32),
    Err(_) => None,
  }      
}

// get post by id
pub async fn get_post(id: i32) -> Option<Post> {
  let uri = &env::var("DATABASE_URL").unwrap();
  let pool = MySqlPool::connect(uri).await.unwrap();
  let post = sqlx::query_as::<_, Post>(
    r#"
    SELECT 
      blogapp_post.id as id, 
      title,
      blogapp_category.name as category,
      contents, 
      pub_date,
      open
    FROM
      blogapp_post
    INNER JOIN 
      blogapp_category 
    ON
      blogapp_post.category_id = blogapp_category.id
    WHERE
      blogapp_post.id = ?
    "#, 
  )
  .bind(id)
  .fetch_one(&pool)
  .await;

  match post {
    Ok(post) => Some(post),
    // sqlx::Error::RowNotFound の時はエラーにせずに空の値を返す
    Err(_) => Some(
      Post {
        id: 0,
        title: "".to_string(),
        category: None,
        contents: None,
        pub_date: Utc::now(),
        open: 0,
      }
    ),
  }
}

// get posts by page and category
pub async fn get_posts(page: i32, category: String) -> anyhow::Result<Vec<Post>> {
  let pool = MySqlPool::connect(&env::var("DATABASE_URL")?).await?;
  let offset = if page == 0 { 0 } else { 5 * (page - 1) };
  let category_query = if category == "" {
    format!("{}", "")
  } else {
    format!("AND blogapp_category.name = '{}'", category)
  };

  let sql = format!(
    "
    SELECT 
      blogapp_post.id, 
      title, 
      blogapp_category.name as category, 
      left(contents, 200) as contents, 
      pub_date,
      open
    FROM 
      blogapp_post 
    INNER JOIN
      blogapp_category 
    ON
      blogapp_post.category_id = blogapp_category.id
    WHERE 
      open = true
      {}
    ORDER BY
      blogapp_post.pub_date desc  
    LIMIT 5
    OFFSET ?
    ",
    category_query
  );

  let posts = sqlx::query_as::<_, Post>(
    sql.as_str(), 
  )
  .bind(offset)
  .fetch_all(&pool)
  .await?;

  Ok(posts)
}
