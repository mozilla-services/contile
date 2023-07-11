#!/bin/bash
set -eu

# Declare variables
GCLOUD=$(which gcloud)
SED=$(which sed)
KUBECTL=$(which kubectl)
GOOGLE_CLOUD_PROJECT=$(gcloud config get-value project)
CLUSTER='contile-locust-load-test'
TARGET='https://contile-stage.topsites.nonprod.cloudops.mozgcp.net'
SCOPE='https://www.googleapis.com/auth/cloud-platform'
REGION='us-west1'
WORKER_COUNT=5
MACHINE_TYPE='n1-standard-2'
BOLD=$(tput bold)
NORM=$(tput sgr0)
DIRECTORY=$(pwd)

CONTILE_DIRECTORY=$DIRECTORY/kubernetes-config
MASTER_FILE=locust-master-controller.yml
WORKER_FILE=locust-worker-controller.yml
SERVICE_FILE=locust-master-service.yml

LOCUST_IMAGE_TAG=$(git log -1 --pretty=format:%h)
echo "Image tag for locust is set to: ${LOCUST_IMAGE_TAG}"

# Declare variables to be replaced later in the YAML file using the sed commands
ENVIRONMENT_VARIABLES=(
  "TARGET_HOST,$TARGET"
  'LOCUST_CSV,contile'
  "LOCUST_HOST,$TARGET"
  'LOCUST_USERS,"200"'
  'LOCUST_SPAWN_RATE,"3"'
  'LOCUST_RUN_TIME,"600"' # 10 minutes
  'LOCUST_LOGLEVEL,INFO'
  'CONTILE_LOCATION_TEST_HEADER,X-Test-Location'
)

SetEnvironmentVariables()
{
  filePath=$1
  for e in "${ENVIRONMENT_VARIABLES[@]}"
  do
      IFS="," read name value <<< "$e"
      if [ -z "$value" ]; then
        echo -e "\033[33mWARNING! The $name environment variable is undefined\033[0m"
        continue
      fi
      $SED -i -e "/name: $name/{n; s|value:.*|value: $value|}" $filePath
  done
}

SetupGksCluster()
{

    #Configure Kubernetes
    echo -e "==================== Prepare environments with set of environment variables "
    echo -e "==================== Set Kubernetes Cluster "
    export CLUSTER=$CLUSTER
    echo -e "==================== Set Kubernetes TARGET "
    export TARGET=$TARGET
    echo -e "==================== Set SCOPE "
    export SCOPE=$SCOPE

    echo -e "==================== Refresh Kubeconfig at path ~/.kube/config "
    $GCLOUD container clusters get-credentials $CLUSTER --region $REGION --project $GOOGLE_CLOUD_PROJECT

    ##Build Docker Images
    echo -e "==================== Build the Docker image and store it in your project's container registry. Tag with the latest commit hash "
    $GCLOUD builds submit --tag gcr.io/$GOOGLE_CLOUD_PROJECT/locust-contile:$LOCUST_IMAGE_TAG
    echo -e "==================== Verify that the Docker image is in your project's container repository"
    $GCLOUD container images list | grep locust-contile

    ##Deploying the Locust master and worker nodes
    echo -e "==================== Update Kubernetes Manifests "
    echo -e "==================== Replace the target host, project ID and environment variables in the locust-master-controller.yml and locust-worker-controller.yml files"

    $SED -i -e "s|replicas:.*|replicas: $WORKER_COUNT|" $CONTILE_DIRECTORY/$WORKER_FILE
    for file in $MASTER_FILE $WORKER_FILE
    do
        $SED -i -e "s|image:.*|image: gcr.io/$GOOGLE_CLOUD_PROJECT/locust-contile:$LOCUST_IMAGE_TAG|" $CONTILE_DIRECTORY/$file
        SetEnvironmentVariables $CONTILE_DIRECTORY/$file
    done

    ##Deploy the Locust master and worker nodes using Kubernetes Manifests
    echo -e "==================== Deploy the Locust master and worker nodes"
    $KUBECTL apply -f $CONTILE_DIRECTORY/$MASTER_FILE
    $KUBECTL apply -f $CONTILE_DIRECTORY/$SERVICE_FILE
    $KUBECTL apply -f $CONTILE_DIRECTORY/$WORKER_FILE

    echo -e "==================== Verify the Locust deployments & Services"
    $KUBECTL get pods -o wide
    $KUBECTL get services
}

echo "==================== The script is used to create & delete the GKE cluster"
echo "==================== Do you want to create or setup the existing or delete GKE cluster? Select ${BOLD}create or delete or setup ${NORM}"
while :
do
    read response
    case $response in
        create) #Setup Kubernetes Cluster
            echo -e "==================== Creating the GKE cluster "
            $GCLOUD container clusters create $CLUSTER --region $REGION --scopes $SCOPE --enable-autoscaling --min-nodes "5" --max-nodes "10" --scopes=logging-write,storage-ro --addons HorizontalPodAutoscaling,HttpLoadBalancing  --machine-type $MACHINE_TYPE
            SetupGksCluster
            break
            ;;
        delete)
            echo -e "==================== Delete the GKE cluster "
            $GCLOUD container clusters delete $CLUSTER --region $REGION
            break
            ;;
        setup)
            echo -e "==================== Setup the GKE cluster "
            SetupGksCluster
            break
            ;;
        *)
            echo -e "==================== Incorrect input! "
            break
            ;;
    esac
done
