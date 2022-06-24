use chrono::{DateTime, Utc};
use sqlx::{mysql::MySqlPool};
use std::env;

use async_graphql::{
  Object,
  Context,
  SimpleObject,
  ErrorExtensions, 
  FieldError, FieldResult, ResultExt, 
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
    #[error("投稿が存在しません")]
    NotFoundPost,

    #[error("投稿が存在しません")]
    NotFoundPosts,

    #[error("ServerError")]
    ServerError(String),

    #[error("No Extensions")]
    ErrorWithoutExtensions,
}

impl ErrorExtensions for BlogError {
  fn extend(&self) -> FieldError {
      self.extend_with(|err, e| match err {
        BlogError::NotFoundPost => e.set("code", "NOT_FOUND"),
        BlogError::NotFoundPosts => e.set("code", "NOT_FOUND"),
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
  ) -> FieldResult<Post> {
    let post = get_post(id).await;
    match post.id {
      0 => return Err(BlogError::NotFoundPost).extend(),
      _ => Ok(post),
    }
  }

  #[allow(non_snake_case)]
  async fn getPosts(
      &self, 
      _ctx: &Context<'_>,
      #[graphql(desc = "current page")] page: i32, 
      #[graphql(desc = "selected category")] category: String
  ) -> FieldResult<Posts> {
    let convertPage = if page == 0 { 1 } else { page };
    let categoryForResult = category.clone();
    let count =  match count().await {
      0 => return Err(BlogError::NotFoundPosts).extend(),
      c => c,
    };

    let results = get_posts(page, category).await;
    // let results = match posts {
    //   [] => Err(BlogError::NotFoundPosts.into()),
    //   posts => Ok(posts),
    // };

    let page_size = (count / 5) + 1;
    let posts = Posts {
        current: convertPage,
        next: if convertPage == page_size { Some(page_size) } else { Some(convertPage + 1) },
        prev: if convertPage == 0 { Some(0) } else { Some(convertPage - 1) },
        category: categoryForResult,
        page_size,
        results,
    };
    Ok(posts)
}

  async fn extend_result(&self) -> FieldResult<Post> {
      Err(BlogError::NotFoundPost).extend()
  }

  async fn extend_results(&self) -> FieldResult<Post> {
    Err(BlogError::NotFoundPosts).extend()
  }

  async fn extend_server_error(&self) -> FieldResult<Post> {
    Err(BlogError::ServerError("ServerError".to_string())).extend()
  }
}

/**
 * database
 */

enum DatabaseError {
  ServerError(String),
  NotFound,
}

async fn pool () -> Result<MySqlPool, DatabaseError> {
  let url = match env::var("DATABASE_URL") {
    Ok(url) => url,
    Err(_) => {
      return Err(DatabaseError::ServerError("DATABASE_URL is not set".to_string()));
    }
  };
  let pool = MySqlPool::connect(&url).await;
  match pool {
    Ok(pool) => Ok(pool),
    Err(e) => return Err(DatabaseError::ServerError(e.to_string())),
  }
}

// count all posts
pub async fn count() -> i32 {
  let uri = &env::var("DATABASE_URL").unwrap();
  let pool = MySqlPool::connect(uri).await.unwrap();
  let count_all = sqlx::query_as::<_, Count>(
    r#"
SELECT count(*) as count FROM blogapp_post where open = true
    "#
)
  .fetch_one(&pool)
  .await;

  // open = true の投稿なかったら勝手に0になるから無理にハンドリングする必要ないけど一応
  match count_all {
    Ok(count) => count.count as i32,
    Err(_) => 0,
  }
}

// get post by id
pub async fn get_post(id: i32) -> Result<Post, DatabaseError> {
  let pool = match pool().await {
    Ok(pool) => pool,
    Err(_) => return Err(DatabaseError::ServerError("Database Error: connection failed".to_string())),
  };

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
  
  match post.unwrap().id {
    0 => Err(DatabaseError::NotFound),
    _ => Ok(post.unwrap()),
  }
}

// get posts by page and category
pub async fn get_posts(page: i32, category: String) -> Result<Vec<Post>, DatabaseError> {
  let pool = match pool().await {
    Ok(pool) => pool,
    Err(_) => return Err(DatabaseError::ServerError("Database Error: connection failed".to_string())),
  };

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
  .await;

  match posts {
    Ok(posts) => Ok(posts),
    Err(_) => Err(DatabaseError::NotFound),
  }
}
