#!/bin/bash

source .env

if [ "${k8s_provider}" == "eks" ]; then
    echo "Updating your kube context locally ...."
    aws eks update-kubeconfig --name ${TF_VAR_eks_cluster_name}
    cd ../charts
    echo "Deploying fuel-core helm chart to ${TF_VAR_eks_cluster_name} ...."
    helm upgrade fuel-core . \
              --values values.yaml \
              --install \
              --create-namespace \
              --namespace=fuel-core \
              --wait \
              --timeout 8000s \
              --debug
else
   echo "You have chosen a non-supported kubernetes provider"
fi
