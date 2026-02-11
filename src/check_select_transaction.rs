use crate::db::firestore::FirestoreDb;

pub async fn check_select_transaction(db: &FirestoreDb) {
    let mut transaction = db.get_client().unwrap().begin_transaction().await.unwrap();
    let _ = db.get_client().unwrap()
        .fluent()
        .select()
        .by_id_in("test")
        .obj::<()>()
        .one("id")
        //.transaction(&mut transaction) // Uncomment to check
        .await;
}
