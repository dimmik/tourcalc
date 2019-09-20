export default class FetchHelper {
    static fetchTours(comp) {
        fetch('/api/tour')
            .then(res => res.json())
            .then((result) => {
                comp.setState({
                    isLoaded: true,
                    tours: result
                });
            },
                (error) => {
                    comp.setState({
                        isLoaded: true,
                        error
                    });
                });
    }
    static fetchTourCalculated(comp, tourid) {
        fetch('/api/tour/' + tourid + '/calculated')
            .then(res => res.json())
            .then(
                (result) => {
                    comp.setState({
                        isLoaded: true,
                        tour: result
                    });
                },
                (error) => {
                    comp.setState({
                        isLoaded: true,
                        error
                    });
                })
    }

}