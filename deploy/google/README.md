# Google Cloud Run Deployment

This deployment runs the normal Axum server on Cloud Run and stores link records
in Firestore. Cloud Run can scale the container to zero while Firestore remains
the durable state.

Prerequisites:

```bash
gcloud services enable run.googleapis.com artifactregistry.googleapis.com firestore.googleapis.com secretmanager.googleapis.com
gcloud firestore databases create --location=nam5
gcloud iam service-accounts create zhorten-cloud-run
gcloud projects add-iam-policy-binding "$GOOGLE_CLOUD_PROJECT" \
  --member="serviceAccount:zhorten-cloud-run@$GOOGLE_CLOUD_PROJECT.iam.gserviceaccount.com" \
  --role="roles/datastore.user"
printf '%s' 'choose-a-long-random-password' | gcloud secrets create zhorten-password --data-file=-
gcloud secrets add-iam-policy-binding zhorten-password \
  --member="serviceAccount:zhorten-cloud-run@$GOOGLE_CLOUD_PROJECT.iam.gserviceaccount.com" \
  --role="roles/secretmanager.secretAccessor"
```

Build and push the image:

```bash
gcloud artifacts repositories create zhorten --repository-format=docker --location=us
docker build -f deploy/google/Dockerfile -t us-docker.pkg.dev/$GOOGLE_CLOUD_PROJECT/zhorten/zhorten-google:latest .
docker push us-docker.pkg.dev/$GOOGLE_CLOUD_PROJECT/zhorten/zhorten-google:latest
```

Deploy with explicit scale-to-zero and single-instance limits:

```bash
gcloud run deploy zhorten \
  --image us-docker.pkg.dev/$GOOGLE_CLOUD_PROJECT/zhorten/zhorten-google:latest \
  --region us-central1 \
  --service-account zhorten-cloud-run@$GOOGLE_CLOUD_PROJECT.iam.gserviceaccount.com \
  --min-instances 0 \
  --max-instances 1 \
  --concurrency 80 \
  --set-env-vars ZHORTEN_USERNAME=admin,ZHORTEN_GOOGLE_PROJECT=$GOOGLE_CLOUD_PROJECT,ZHORTEN_FIRESTORE_COLLECTION=links,ZHORTEN_SECURE_COOKIES=true \
  --set-secrets ZHORTEN_PASSWORD=zhorten-password:latest \
  --allow-unauthenticated
```

Useful checks:

```bash
gcloud run services describe zhorten --region us-central1
gcloud run services logs read zhorten --region us-central1
```

The initial template deliberately caps Cloud Run at one instance for the
single-process teaching demo. Firestore supports shared storage, so the cap can
be raised later after deciding how much horizontal write contention and cost
exposure the deployment should allow.
