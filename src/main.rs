
use anyhow::anyhow;
//use axum::Extension;
//use axum::extract::Path;
use axum::{
    routing::{get, post, put, delete},
    http::{StatusCode, header, HeaderMap, Method},
    Router,
    response::IntoResponse,
    Json,    
    Extension,
    extract::{Path, Query, connect_info::IntoMakeServiceWithConnectInfo}, 
};

use hyper::header::{HeaderName, CONTENT_TYPE};
use sqlx::postgres::{PgPoolOptions, PgRow, PgQueryResult};
use sqlx::{PgPool, query};
use std::net::SocketAddrV4;
use serde_json::{Value, json};
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};
use dotenv::dotenv;

//там функция escape_internal, чтобы обезопаситься от SQLi
mod lib;

mod migrations {
    use anyhow::anyhow;
    use diesel::{PgConnection, Connection};

    pub mod my_db {
        use anyhow::anyhow;
        use diesel_migrations::{EmbeddedMigrations, MigrationHarness};

        pub const MIGRATIONS: EmbeddedMigrations = diesel_migrations::embed_migrations!("migrations");

        pub fn apply(database_url: &str) -> anyhow::Result<()> {
            let mut conn = super::migrations_connection(database_url)?;
            conn.run_pending_migrations(MIGRATIONS).map_err(|e| anyhow!("running pending migrations: {e}"))?;
            Ok(())
        }
    }    

    pub fn migrations_connection(database_url: &str) -> anyhow::Result<PgConnection> {
        PgConnection::establish(database_url)
            .map_err(|e| anyhow!("connecting to database: {e}"))
    }

    

   
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {    
    dotenv().ok();

    env_logger::init();

    let ip = std::env::var("IP").expect("Переменная IP не найдена");
    
    let database_url = std::env::var("DATABASE_URL").expect("Переменная DATABASE_URL не найдена");

    log::debug!("Applying 'my_db' migrations on '{database_url}'...");

    migrations::my_db::apply(&database_url).map_err(|e| {
        anyhow!("applying 'my_db' migrations on '{database_url}': {e}")
    })?;

    log::info!("Successfully applied 'my_db' migrations on '{database_url}'");

    let socet: SocketAddrV4 = ip.parse().expect("Не смог распарсить IP и socet"); 

    log::debug!("Creating connection pool to '{database_url}'...");
    let pool = PgPoolOptions::new()
        .max_connections(50)
        .connect(&database_url)
        .await
        .unwrap();
       
    log::info!("Successfully created connection pool to '{database_url}'");

    let app = Router::new()
    // тест. Hello? world
    .route("/", get(hello))    
    // .layer(tower_http::cors::CorsLayer::new()
        // .allow_origin("*".parse::<axum::http::HeaderValue>().unwrap())
        // .allow_headers([CONTENT_TYPE])
        // .allow_methods([axum::http::Method::GET]), )
    // GET запрос на получение всех авторов
    .route("/api/v1/authors", get(get_authors))
    // GET запрос на поиск автора по имени
    .route("/api/v1/author/search", get(search_author))
    // GET запрос на получение книг по author_id автора
    .route("/api/v1/author/:author_id", get(get_author_name))
    // GET запрос на поиск количество авторов по критерию
    .route("/api/v1/author/search/count", get(author_search_count))
    // PUT запрос на изменение имени атора
    .route("/api/v1/author/:author_id", put(update_author_name))
    // POST запрос на создание автора
    .route("/api/v1/author", post(add_author)) 
    // POST запрос на создание автора
    .route("/api/v1/author2", post(add_author_2)) 
    // DELETE запрос на удаление автора
    .route("/api/v1/author/:author_id", delete(delete_author))        
    .layer(Extension(pool))
    .layer(tower_http::cors::CorsLayer::new()
        .allow_origin("*".parse::<axum::http::HeaderValue>().unwrap())
        .allow_headers([CONTENT_TYPE])
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::PUT]), );
       

    let addr = SocketAddr::from(socet);   
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap(); 

    Ok(())
}

async fn hello() -> Json<String>{    
    let message = "Hello, world".to_string();
    Json(message)
}

// POST запрос: добывить нового автора
async fn  add_author(Extension(pool): Extension<PgPool>, Json(author_name): Json<NewAuthor> ) -> Result<(StatusCode, Json<NewAuthor>), CustomError>{       

    if author_name.author_name.is_empty() {
        return Err(CustomError::BadRequest)
    }

    let sql = "INSERT INTO authors(name) VALUES ($1)".to_string();
    
    let _ = sqlx::query(&sql)
        .bind(&author_name.author_name)
        .execute(&pool)
        .await
        .map_err(|_| {
            CustomError::AuthorIsRepeats
        })?;    
            
    Ok((StatusCode::CREATED, Json(author_name)))
} 

// POST2 запрос: добывить нового автора
async fn  add_author_2(Extension(pool): Extension<PgPool>, Json(author): Json<NewAuthor2> ) -> Result<(StatusCode, Json<NewAuthor2>), CustomError>{       

    if author.authors_name.is_empty() {
        return Err(CustomError::BadRequest)
    }

    println!("author.authors_name = {}, author.adress = {}", author.authors_name, author.adress);

    let sql = "INSERT INTO authors2(authors_name, adress) VALUES ($1, $2)".to_string();
    
    let _ = sqlx::query(&sql)
        .bind(&author.authors_name)
        .bind(&author.adress)
        .execute(&pool)
        .await
        .map_err(|_| {
            CustomError::AuthorIsRepeats
        })?;    
            
    Ok((StatusCode::CREATED, Json(author)))
} 

// GET запрос: получить список всех авторов
async fn get_authors(Extension(pool): Extension<PgPool>) -> impl IntoResponse {
       
    let sql = "SELECT * FROM authors".to_string();

    let list_authors = sqlx::query_as::<_, Author>(&sql)
        .fetch_all(&pool)
        .await
        .unwrap();
       
    //(StatusCode::OK, [(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")], Json(list_authors))  
    (StatusCode::OK, Json(list_authors)) 
}

// PUT запрос: изменение имени автора по id //
async fn update_author_name(Path(author_id): Path<i32>, Extension(pool): Extension<PgPool>, Json(update_author): Json<NewAuthor>) -> Result<StatusCode, CustomError> {
  
   // открываем транзакцию
   let mut transaction = pool.begin().await.unwrap();   

    let _find: Author = sqlx::query_as("SELECT * FROM authors WHERE authors_id=$1")
       .bind(author_id)
       .fetch_one(&mut transaction)
       .await
       .map_err(|_| {
            CustomError::AuthorNotFound
       })?;
        
    let sql = "UPDATE authors SET name=$1 WHERE authors_id=$2".to_string();   

    let _ = sqlx::query(&sql)
        .bind(&update_author.author_name)
        .bind(author_id)
        .execute(&mut transaction)         
        .await
        .map_err(|_| {
             CustomError::InternalServerError
        })?; 

   // закрываем транзакцию    
   transaction.commit().await.unwrap();

    // let mut headers = HeaderMap::new();
    // headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());            
    //Ok((StatusCode::OK, headers))
    Ok(StatusCode::OK)
 }

// DELETE запрос: удаление автора по id
async fn delete_author(Path(author_id): Path<i32>, Extension(pool): Extension<PgPool>) -> Result<(StatusCode, Json<Value>), CustomError> {

    // открываем транзакцию
    let mut transaction = pool.begin().await.unwrap();    
    
    let _find: Author = sqlx::query_as("SELECT * FROM authors WHERE authors_id=$1")
        .bind(author_id)
        .fetch_one(&mut transaction)
        .await
        .map_err(|_| {
            CustomError::AuthorNotFound            
        })?;

    let sql = "DELETE FROM authors WHERE authors_id=$1".to_string();
 
    sqlx::query(&sql)        
         .bind(author_id)
         .execute(&mut transaction)         
         .await
         .map_err(|_| {
            CustomError::InternalServerError
         })?; 
 
    // закрываем транзакцию    
    transaction.commit().await.unwrap();

    // отправляю дополнительно заголовок, для того чтобы браузер не блокировал входящий json
    // let mut headers = HeaderMap::new();
    // headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
    // Ok((StatusCode::OK, headers, Json(json!({"msg": "Author Deleted"}))))
    
    Ok((StatusCode::OK, Json(json!({"msg": "Author Deleted"}))))
 }

// GET запрос: поиск автора по имени
async fn search_author(  Extension(pool): Extension<PgPool>, Query(query): Query<NewAuthor>) ->  Result<(StatusCode, Json<Vec<Author>>), CustomError> { 
    
    // sql-запрос
    let mut sql = "SELECT * FROM authors WHERE name LIKE ".to_string(); 
    // пропускаем через функцию escape_internal параметр запроса и URL, чтобы обезопаситься от SQLi
    let query_param = lib::escape_internal(&query.author_name, false); 
    // добавляем получившийся параметр запроса к SQL-запросу
    sql.push_str(&query_param);      

    let author: Vec<Author> = sqlx::query_as::<_, Author>(&sql)          
        .fetch_all(&pool)
        .await         
        .map_err(|_| {
           CustomError::InternalServerError
        })?;     
    
    // если в БД нет совпадений, то вернём ошибку об отсутствии таких авторов
    if author.is_empty() {
        return Err(CustomError::AuthorNotFound)
    }   

    // отправляю дополнительно заголовок, для того чтобы браузер не блокировал входящий json
    // let mut headers = HeaderMap::new();
    // headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());    
    // Ok((StatusCode::OK,headers, Json(author)))
    Ok((StatusCode::OK, Json(author)))
 }

 // GET запрос: посчитать количество авторов по критериям
async fn author_search_count(  Extension(pool): Extension<PgPool>, Query(query): Query<QueryParameters>) ->  Result<(StatusCode, Json<i64>), CustomError> { 
    
    // // sql-запрос
    // let mut sql = "SELECT * FROM authors WHERE name LIKE ".to_string(); 
    // // пропускаем через функцию escape_internal параметр запроса и URL, чтобы обезопаситься от SQLi
    // let query_param = lib::escape_internal(&query.author_name, false); 
    // // добавляем получившийся параметр запроса к SQL-запросу
    // sql.push_str(&query_param);      

    let sql = "SELECT COUNT(*) FROM authors WHERE country=$1".to_string();

    let row: (i64,) = sqlx::query_as(&sql) 
        .bind(query.country)         
        .fetch_one(&pool)
        .await         
        .map_err(|_| {
           CustomError::InternalServerError
        })?;     
    println!("{row:?}");

    // // если в БД нет совпадений, то вернём ошибку об отсутствии таких авторов
    // if author.is_empty() {
    //     return Err(CustomError::AuthorNotFound)
    // }   

    // отправляю дополнительно заголовок, для того чтобы браузер не блокировал входящий json
    // let mut headers = HeaderMap::new();
    // headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());    
    // Ok((StatusCode::OK,headers, Json(author)))
    
    
    Ok((StatusCode::OK, Json(row.0)))
 }


// GET запрос: получение атора по id
async fn get_author_name(Path(author_id): Path<i32>, Extension(pool): Extension<PgPool>) -> Result<Json<Author>, CustomError> {
  
    let sql = "SELECT * FROM authors WHERE authors_id=$1".to_string();

    let author = sqlx::query_as::<_, Author>(&sql)
        .bind(author_id)
        .fetch_one(&pool)
        .await
        .map_err(|_| {
            CustomError::AuthorNotFound
        })?;
    
    // отправляю дополнительно заголовок, для того чтобы браузер не блокировал входящий json
    // let mut headers = HeaderMap::new();
    // headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());    
    // Ok((headers, Json(author)))

    Ok(Json(author))
}



#[derive(sqlx::FromRow, Deserialize, Serialize)]
struct NewAuthor {
    author_name: String,
    country: String,
}

#[derive(sqlx::FromRow, Deserialize, Serialize)]
struct NewAuthor2 {
    authors_name: String,
    adress: String,
}

#[derive(sqlx::FromRow, Deserialize, Serialize)]
struct Author {
    authors_id: i32,
    name: String,
    country: String,
}

#[derive(sqlx::FromRow, Deserialize, Serialize)]
struct Book {
    books_id: i32,
    fk_authors_id: i32,
    title: String,
}

#[derive(sqlx::FromRow, Deserialize, Serialize)]
struct NewBook {
    book_name: String,
}

// параметры запроса по различным критериям
#[derive(Debug, Deserialize, Serialize)]
pub struct QueryParameters {   
    country: String,
}

// перечисление для обработки ошибок
enum CustomError {
    BadRequest,
    AuthorNotFound,
    InternalServerError,
    AuthorIsRepeats,
}

// реализуем трейт IntoResponse для enum CustomError
impl IntoResponse for CustomError {
    fn into_response(self) -> axum::response::Response {
    
    // отправляю дополнительно заголовок, для того чтобы браузер не блокировал входящий json
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());

        let (status, error_message) = match self {
            Self::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,                
                "Internal Server Error",
            ),
            Self::BadRequest=> (
                StatusCode::BAD_REQUEST,                  
                "Bad Request"
            ),
            Self::AuthorNotFound => (
                StatusCode::NOT_FOUND,                 
                "Author Not Found"
            ),
            Self::AuthorIsRepeats => (
                StatusCode::NOT_IMPLEMENTED, 
                "The author repeats"
            ),            
        };
        (status, headers, Json(json!({"error": error_message}))).into_response()
    }
}

