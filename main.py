import os
import pickle
import shutil
import time
from functools import lru_cache

import numpy as np
from onnx.compose import merge_models
from skimage.io import imread, imsave
from skimage.transform import resize
from skl2onnx import convert_sklearn
from skl2onnx.common.data_types import FloatTensorType
from sklearn.decomposition import PCA
from sklearn.metrics import accuracy_score, classification_report
from sklearn.model_selection import train_test_split
from sklearn.svm import SVC
from tqdm import tqdm


def train():
    X, y = load_dataset()

    X_train, X_test, y_train, y_test = train_test_split(X, y, train_size=0.9, shuffle=True)

    pca = PCA(n_components=150)
    pca.fit(X_train)
    X_train = pca.transform(X_train)
    X_test = pca.transform(X_test)

    classifier = SVC(verbose=True, tol=1e-6, class_weight="balanced")

    classifier.fit(X_train, y_train)

    y_prediction = classifier.predict(X_test)

    score = accuracy_score(y_test, y_prediction)
    report = classification_report(y_test, y_prediction)
    print(report)

    print(f"{score * 100:.2f}% of samples were correctly classified")

    pickle.dump(pca, open("./pca.p", "wb"))
    pickle.dump(classifier, open("./svm.p", "wb"))


def preprocess_img(img_path):
    img = imread(img_path)
    if img.shape == (1080, 1920, 3):
        img = img[750:950, 860:1060]
        imsave(img_path, img)
    img = resize(img, (50, 50))
    if (img > 1).any() or (img < 0).any():
        img = img / 255.0
    return img.flatten()


@lru_cache()
def load_dataset():
    input_dir = "kill-data"
    categories = ["no_kill", "kill"]
    data = []
    labels = []
    for category_idx, category in enumerate(categories):
        files = os.listdir(os.path.join(input_dir, category))
        for file in tqdm(files, desc=category):
            img_path = os.path.join(input_dir, category, file)
            img = preprocess_img(img_path)
            if img is None:
                continue
            data.append(img)
            labels.append(category_idx)
    return np.asarray(data), np.asarray(labels)


def benchmark():
    X, y = load_dataset()
    pca = pickle.load(open("./pca.p", "rb"))
    model = pickle.load(open("./svm.p", "rb"))
    times = []
    X = pca.transform(X)
    # warmup
    for img in X[:10]:
        model.predict([img])
    for img in tqdm(X[:1000], desc="Benchmarking"):
        start = time.time()
        model.predict([img])
        end = time.time()
        times.append(end - start)

    print(f"Average inference time: {np.mean(times) * 1000:.2f}ms")


def infer():
    pca = pickle.load(open("./pca.p", "rb"))
    svm = pickle.load(open("./svm.p", "rb"))

    base_dir = "kill-data/kill"
    for file in tqdm(os.listdir(base_dir)):
        img = preprocess_img(os.path.join(base_dir, file))
        img = pca.transform([img])[0]
        prediction = svm.predict([img])
        if prediction != 1:
            shutil.move(os.path.join(base_dir, file), os.path.join("kill-data/wrong", file))
            print(f"Wrong prediction for {file}")

    # base_dir = "kill-data/no_kill"
    # for file in tqdm(os.listdir(base_dir)):
    #     img = preprocess_img(os.path.join(base_dir, file))
    #     img = pca.transform([img])[0]
    #     prediction = svm.predict([img])
    #     if prediction != 0:
    #         shutil.move(os.path.join(base_dir, file), os.path.join("kill-data/wrong", file))
    #         print(f"Wrong prediction for {file}")


def export(input_size: int = 50 * 50 * 3):
    pca = pickle.load(open("./pca.p", "rb"))
    svm = pickle.load(open("./svm.p", "rb"))

    pca_input = [("pca_input", FloatTensorType([None, input_size]))]
    pca_output = [("pca_output", FloatTensorType([None, pca.transform([[0] * input_size]).shape[1]]))]
    svm_input = [("svm_input", FloatTensorType([None, pca.transform([[0] * input_size]).shape[1]]))]

    pca_onnx = convert_sklearn(pca, initial_types=pca_input, final_types=pca_output, target_opset=18, model_optim=True)
    svm_onnx = convert_sklearn(svm, initial_types=svm_input, target_opset=18, model_optim=True)

    pca_onnx.opset_import[0].version = 9

    onnx = merge_models(pca_onnx, svm_onnx, io_map=[
        ("pca_output", "svm_input")
    ])
    remove_outputs = (o for o in onnx.graph.output if
                      o.type.WhichOneof("value") == "sequence_type" or o.name == "probabilities")
    for o in remove_outputs:
        onnx.graph.output.remove(o)
    remove_nodes = (n for n in onnx.graph.node if n.output[0] == "probabilities")
    for n in remove_nodes:
        onnx.graph.node.remove(n)
    with open("model.onnx", "wb") as f:
        f.write(onnx.SerializeToString())


if __name__ == "__main__":
    train()
    benchmark()
    # infer()
    export()
